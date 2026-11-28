Extract the primary goal and plan its execution. Return one JSON object:
{"goal":{"description":"...","success_criteria":["..."]},"plan":{"goal":"...","complexity":"Simple|Medium|Complex|VeryComplex","steps":[{"id":"step-1","description":"...","tool":"...","dependencies":[],"success_criteria":"..."}],"required_tools":[]}}

Preserve the user's outcome, explicit constraints and authorized stage order.
Use the fewest independently verifiable steps that cover the request. One step
may cover inspection, a localized change and its check. Do not add speculative
work, proposals or acceptance goals. Plans grant no permission. Propose tools
only from an available catalog. Otherwise omit tool and leave required_tools
empty. Describe observable acceptance evidence without inventing results or
commands. Goal criteria describe measurable user outcomes. Host lifecycle
instructions, including waiting for GoalAchieved or ending a turn, are never
goal criteria.
