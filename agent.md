# WORKFLOW: gitar plan — layered single-shot flow

Layers:
  UI -> Kernel -> Skill -> Context -> Planner
     -> LLM Client -> Policy -> Executor -> Telemetry/Audit -> Output

------------------------------------------------------------

Agent.main(argv):
  cmd = Agent.parse(argv)
  cfg = Agent.config()
  return Kernel.run(cmd, cfg)

------------------------------------------------------------

Kernel.run(cmd, cfg):
  ctx = Context(
    cwd = OS.cwd(),
    cfg = cfg,
    budget = TokenBudget.from_cfg(cfg),
    policy = Policy.default(cfg),
    telemetry = Telemetry.open_jsonl(".gitar/audit.jsonl"),
  )

  skill = Skill.resolve(cmd.verb)
  ctx.telemetry.emit(Skill.event(skill.name, cmd.args, 'start'))

  result = skill.run(ctx, cmd.args) // no loops, single-shot to LLM

  ctx.telemetry.emit(Skill.event(skill.name, result.summary, 'end'))
  UI.render(result)
  return result.exit_code

------------------------------------------------------------

Skill.run(ctx, args):
  // 1) Build planning context (deterministic)
  ctx.telemetry.emit(ContextBuildStart(kind="planning"))

  repo      = GitRepo.open_or_fail(ctx.cwd)
  git_state = repo.snapshot_state()                   // branch, HEAD, dirty, etc.
  raw_diff  = repo.diff_for_planning(args.scope)      // staged/unstaged/range
  files     = DiffAnalyzer.extract_files(raw_diff)

  signals = Detect.run_all(
    raw_diff,
    files,
    repo.name_status(),                               // for rename signals
  )

  planning_ctx = PlanningContext(
    git_state = git_state,
    files = files,                                   // (path, kind, churn, renamed?, tags)
    signals = signals,
    constraints = DefaultConstraints.from_cfg(ctx.cfg, signals),
    objectives  = DefaultObjectives.from_cfg(ctx.cfg),
    token_budget = ctx.budget.for_task("plan"),
  )

  // policy guard on context (secrets/huge diffs/blocked paths)
  ctx.policy.check_context(planning_ctx)              // may require confirm/switch modes
  planning_ctx = ctx.policy.apply_redactions(planning_ctx)

  ctx.telemetry.emit(ContextBuildEnd(kind="planning", stats=planning_ctx.stats))

  // 2) Choose planner path (LLM optional)
  if PlannerHeuristics.is_trivial(planning_ctx):
    plan = HeuristicPlanner.plan(planning_ctx)
    return SkillResult(plan=plan, mode="heuristic", score=Score.high())

  // 3) LLM planner (default 0–1 call)
  ctx.telemetry.emit(PlanningStart(mode="llm-single-shot"))

  request = PlanRequest(
    schema = "PlanCandidatesV1",
    prompt = Prompts.plan(planning_ctx),
    max_tokens = planning_ctx.token_budget.output_tokens,
    temperature = 0.2,
  )

  candidates = LLMPlatform.generate_json(ctx, request)
              |> Parser.strict_parse(schema="PlanCandidatesV1")

  ctx.telemetry.emit(PlanningEnd(mode="llm", candidates=len(candidates)))

  // 4) Score & select (deterministic)
  scored = []
  for c in candidates:
    s = PlanScorer.score(c, planning_ctx)
    scored.append((c, s))
    ctx.telemetry.emit(PlanCandidateScored(score=s.total, reasons=s.reasons))

  best, best_score = argmax(scored, key=score.total)

  // 5) Optional bounded repair (2nd call, default OFF)
  if best_score.total < ctx.cfg.plan.min_score and ctx.cfg.plan.allow_repair_call:
    ctx.telemetry.emit(RepairStart(reason=best_score.top_violations))

    repair_req = RepairRequest(
      schema = "PlanCandidateV1",
      prompt = Prompts.repair(planning_ctx, best, best_score.top_violations),
      max_tokens = planning_ctx.token_budget.output_tokens,
      temperature = 0.1,
    )

    repaired = LLMPlatform.generate_json(ctx, repair_req)
              |> Parser.strict_parse(schema="PlanCandidateV1")

    repaired_score = PlanScorer.score(repaired, planning_ctx)
    if repaired_score.total > best_score.total:
      best, best_score = repaired, repaired_score

    ctx.telemetry.emit(RepairEnd(score=best_score.total))

  // 6) Candidate -> executable plan (deterministic)
  plan = PlanBuilder.from_candidate(best, planning_ctx)

  // 7) Final policy guard on plan (destructive/safety/blocklists)
  ctx.policy.check_plan(plan)                          // may require confirm/block

  // 8) Output (plan typically reports, not executes)
  report = PlanReport(
    score = best_score.total,
    reasons = best_score.reasons,
    groups = plan.groups,
    risk = plan.risk,
    next_steps = plan.next_steps,
  )

  return SkillResult(plan=plan, report=report, mode="llm", score=best_score)

------------------------------------------------------------

LLMPlatform.generate_json(ctx, request):
  // reusable “LLM platform layer”
  ctx.telemetry.emit(LLMRequestStart(model=request.model_hint))

  provider = Router.choose_provider(
    preference = ctx.cfg.llm.preference,
    required_caps = ["json_schema"],
    health = ProviderHealthCache.current(),
  )

  creds = Auth.resolve(provider, ctx.cfg, env=OS.env())

  resp = Transport.send_with_resilience(
    provider = provider,
    creds = creds,
    payload = ProviderAdapter(provider).to_payload(request),
    timeout = ctx.cfg.llm.timeout,
    retries = ctx.cfg.llm.retries,
    backoff = "exp_jitter",
    circuit_breaker = ctx.cfg.llm.circuit_breaker,
  )

  norm = ProviderAdapter(provider).from_response(resp)
  norm = OutputPolicy.redact_before_logging(norm, ctx.policy)

  ctx.telemetry.emit(LLMRequestEnd(
    provider = provider.name,
    latency_ms = resp.latency,
    tokens_in = norm.usage.in_tokens,
    tokens_out = norm.usage.out_tokens,
  ))

  // local JSON repair only (no extra call)
  return JsonRepair.try_fix(norm.text)

------------------------------------------------------------

PlanScorer.score(candidate, planning_ctx):
  score = 100
  reasons = []

  // penalties
  if mixes_docs_and_code(candidate):                  score -= 15; reasons += ["mixed docs+code"]
  if mixes_format_and_logic(candidate):              score -= 20; reasons += ["mixed formatting+logic"]
  if violates_rename_rule(candidate, planning_ctx.signals):
                                                      score -= 20; reasons += ["rename mixing"]
  if exceeds_size_caps(candidate, planning_ctx.constraints):
                                                      score -= 10; reasons += ["too large"]
  if touches_many_roots(candidate):                  score -= 10; reasons += ["low cohesion"]

  // bonuses
  if high_subtree_cohesion(candidate):               score += 10
  if tests_paired_with_feature(candidate):           score += 5

  return PlanScore(total=clamp(score, 0, 120), reasons=reasons)

------------------------------------------------------------

PolicyEngine.check_context(planning_ctx):
  if SecretDetector.find(planning_ctx): redact + mark "sensitive"
  if planning_ctx.stats.too_large: require_confirm OR switch_to_heuristic
  if violates_blocklist_paths(planning_ctx): block
  return OK

PolicyEngine.check_plan(plan):
  if plan.contains_destructive_steps and not ctx.cfg.yes: require_confirm
  if plan.violates_blocklist_paths: block
  return OK
