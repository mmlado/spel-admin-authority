# Admin Config PDA seed is "admin_config"

The Config PDA stores admin authority state and is derived from `(program_id, "admin_config")`. We chose a specific seed rather than the generic `"config"` because the macro claims this PDA implicitly. A consumer whose program already uses `pda = literal("config")` for their own settings would suffer a silent data overwrite with no compile-time warning. The seed `"admin_config"` is unambiguous and collision-safe.
