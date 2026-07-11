package com.forge.app.program

/**
 * Canonical exercise — the un-hardlocked replacement for slot-bound [ExercisePlan]s.
 *
 * The library is the **single pool** the generator AND the swap picker draw from (program-unlock
 * Phase 4 — the old `Swaps` catalog was merged in here). Each entry has a **stable** id
 * (kebab-case, e.g. "db-bench-press") independent of which day/slot it lands in, and an explicit
 * [equipment] list that drives availability filtering (#44).
 *
 * [muscleTarget]/[why]/[whenToUse] are the human-readable coaching rationale shown in the swap
 * picker; they're optional so generator-only entries can omit them. [defaultSets]/[defaultReps]
 * are starting points; a placed [ExercisePlan] may override them. Authored in code — no DB
 * seeding migration, easy to expand.
 */
data class ExerciseDef(
    val id: String,
    val name: String,
    val muscle: MuscleGroup,
    /** Equipment required. Empty = no equipment (always available). */
    val equipment: List<Equipment>,
    val unit: ExerciseUnit,
    val tags: List<ExerciseTag> = emptyList(),
    val difficulty: Difficulty = Difficulty.BEGINNER,
    val defaultSets: Int = 3,
    val defaultReps: String = "8-12",
    val formCue: String? = null,
    val note: String = "",
    /** Human-readable description of what this hits, e.g. "Lower lats + biceps assist" (swap picker). */
    val muscleTarget: String? = null,
    /** Why you'd pick this — the rationale shown in the swap picker. */
    val why: String? = null,
    /** Situational guidance ("WHEN") shown in the swap picker. */
    val whenToUse: String? = null,
    /**
     * Last-resort bodyweight fills the generator uses ONLY when no equipped movement covers the
     * muscle (e.g. a bodyweight row on a bodyweight-only setup). Kept out of equipped users' picks so
     * a full-gym lifter never gets handed a bodyweight squat, but guarantees no split is ever starved.
     */
    val fallbackOnly: Boolean = false,
    /**
     * Relative selection weight in generation (1.0 = normal). Below 1 marks niche accessories
     * (adductor work, front raises, bodyweight stand-ins an equipped user wouldn't headline a slot
     * with) — still full swap-picker citizens, just picked less often than the muscle's
     * bread-and-butter movements. Above 1 marks a signature staple (lateral raises — the user's
     * stated priority) that should anchor its slot most weeks.
     */
    val pickBias: Double = 1.0
) {
    /**
     * Owner-only movement — the plate-count ([ExerciseUnit.PLATES]) station exercises, which are
     * specific to Antho's MWM-989 machine (you enter a plate count, not a stack weight). General
     * selectorized machines and cables are entered in lb ([ExerciseUnit.WEIGHT]) instead, so this
     * captures exactly the `mwm-*` stations plus the leg developer's extension/curl. A curated
     * movement appears ONLY inside a curated/frozen preset (the Developer's preset), never in an
     * ordinary equipment-filtered pool — general users get the generically-named lb equivalents.
     * See [ExerciseLibrary.availablePool] and [com.forge.app.program.equipmentPresets].
     */
    val curatedOnly: Boolean get() = unit == ExerciseUnit.PLATES
}

object ExerciseLibrary {

    private val FW = ExerciseTag.FREE_WEIGHT
    private val MC = ExerciseTag.MACHINE
    private val BW = ExerciseTag.BODYWEIGHT
    private val COMP = ExerciseTag.COMPOUND
    private val ISO = ExerciseTag.ISOLATION

    /**
     * Ordered per muscle so the **default / best pick comes first**, then common → niche
     * (the swap picker shows them in this order, like the old `Swaps` catalog did).
     */
    val all: List<ExerciseDef> = listOf(
        // ── Chest ────────────────────────────────────────────────────────────────
        ExerciseDef("db-bench-press", "DB Bench Press", MuscleGroup.CHEST,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "8-10", "1-2 reps shy of failure",
            muscleTarget = "Whole chest (middle and lower)",
            why = "The classic chest builder. Hits the whole muscle and lets you load it heavy as you grow.",
            whenToUse = "Default pick. If you can press dumbbells without shoulder pain, use this."),
        ExerciseDef("incline-db-bench-press", "Incline DB Bench Press", MuscleGroup.CHEST,
            listOf(Equipment.DUMBBELLS, Equipment.INCLINE_BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "8-10", "Fills tee neckline",
            muscleTarget = "Upper chest (the part near your collarbone)",
            why = "Builds the top of your chest, which is what fills out a t-shirt at the neckline.",
            whenToUse = "When your lower chest is catching up but the top still looks flat. Or if regular bench bores you."),
        ExerciseDef("mwm-seated-bench-press", "Seated Bench Press", MuscleGroup.CHEST,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "8-12", "MWM-989 chest press station",
            muscleTarget = "Whole chest, fixed path",
            why = "The MWM-989 seated chest press. The machine guides the path so you can push hard without worrying about balancing the weight.",
            whenToUse = "Your main machine chest press."),
        ExerciseDef("db-fly", "DB Fly", MuscleGroup.CHEST,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "12-15", "Slight elbow bend, stretch then squeeze",
            muscleTarget = "Chest only (stretch + squeeze, no triceps)",
            why = "The only move that isolates the chest with a loaded stretch. Pairs perfectly after any press because tired triceps can't steal the work.",
            whenToUse = "Second chest movement after a press, or when your triceps are fried."),
        ExerciseDef("push-up", "Push-Up", MuscleGroup.CHEST,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.BEGINNER, 3, "AMRAP", "Feet elevated to make it harder",
            muscleTarget = "Whole chest + shoulders + triceps",
            why = "No equipment needed. Feet on a bench makes it harder than regular push-ups.",
            whenToUse = "Equipment is in use, or as a warm-up before pressing.",
            fallbackOnly = true),

        // ── Back ─────────────────────────────────────────────────────────────────
        ExerciseDef("mwm-wide-lat-pulldown", "Wide Lat Pulldown", MuscleGroup.BACK,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 4, "8-12", "Wide grip, pull to upper chest",
            muscleTarget = "Lats (the wing muscles on the sides of your back)",
            why = "The MWM-989 lat pulldown. Builds back WIDTH — half of the V-taper. Easier to learn than pull-ups.",
            whenToUse = "Your main machine back exercise."),
        ExerciseDef("db-row", "DB Row (1-arm)", MuscleGroup.BACK,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 4, "8-12", "Knee on bench, row to hip",
            muscleTarget = "One side of your back at a time",
            why = "Fixes left/right imbalance. Place a knee on the bench, other foot on floor, row the DB to your hip.",
            whenToUse = "You notice one side does more work than the other. Or as a finisher."),
        ExerciseDef("chest-supported-db-row", "Chest-Supported DB Row", MuscleGroup.BACK,
            listOf(Equipment.DUMBBELLS, Equipment.INCLINE_BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "10-12", "Lie chest-down on an incline bench",
            muscleTarget = "Mid-back, both sides, no lower-back strain",
            why = "Lying chest-down on an incline bench stops you swinging — all the work goes to your back, none to momentum or your lower back.",
            whenToUse = "You want rows without loading the lower back, or you keep heaving single-arm rows."),
        // (Seated Low Row and Face Pull were removed 2026-06-10: they claimed MWM-989 stations
        //  Antho's machine can't actually perform — same philosophy as the barbell removal, only
        //  model movements the real gear supports.)
        ExerciseDef("pull-up", "Pull-Up", MuscleGroup.BACK,
            listOf(Equipment.PULL_UP_BAR), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.ADVANCED, 3, "AMRAP", "Full hang to chin over the bar",
            muscleTarget = "Whole back + biceps",
            why = "King of bodyweight back exercises if you have a pull-up bar. Builds insane back density.",
            whenToUse = "You have a pull-up bar and can do at least 3-5 clean reps."),
        ExerciseDef("chin-up", "Chin-Up", MuscleGroup.BACK,
            listOf(Equipment.PULL_UP_BAR), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.ADVANCED, 3, "AMRAP", "Underhand grip — biceps assist",
            muscleTarget = "Whole back + biceps (more bicep)",
            why = "Underhand grip lets the biceps assist, so most people get more reps than on pull-ups. Builds back and arms together.",
            whenToUse = "You can't quite get full pull-ups yet, or you want more bicep involvement."),
        ExerciseDef("mwm-close-grip-pulldown", "Close-Grip Lat Pulldown", MuscleGroup.BACK,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "10-12", "Elbows tight, pull to upper chest",
            muscleTarget = "Lower lats + biceps assist",
            why = "Narrow grip on the MWM-989 lat bar shifts the work to the lower lats and usually lets you pull more weight than the wide grip.",
            whenToUse = "Second pulldown variation — alternate with the wide grip across the week."),
        ExerciseDef("mwm-straight-arm-pulldown", "Straight-Arm Pulldown", MuscleGroup.BACK,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Arms straight, sweep the bar to your thighs",
            muscleTarget = "Lats only (no biceps)",
            why = "MWM-989 high pulley. Arms locked straight, sweep the bar down to your thighs — pure lat work with zero biceps, so the back does everything.",
            whenToUse = "Lat finisher, or when your biceps give out before your back does.",
            pickBias = 0.7),
        ExerciseDef("db-pullover", "DB Pullover", MuscleGroup.BACK,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.INTERMEDIATE, 3, "10-12", "Lower the DB behind your head, big stretch",
            muscleTarget = "Lats + chest (deep stretch)",
            why = "Lying across the bench, lower one DB behind your head and pull it back over. Old-school size builder with a huge lat stretch.",
            whenToUse = "Variety pick when rows and pulldowns feel stale.",
            pickBias = 0.6),

        // ── Shoulders ──────────────────────────────────────────────────────────────
        ExerciseDef("db-lateral-raise", "DB Lateral Raise", MuscleGroup.SHOULDERS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 4, "12-15", "Priority — slow eccentric",
            muscleTarget = "Side delts (the cap on top of your shoulder that makes shoulders look WIDE)",
            why = "The single best exercise for shoulder width. Big visual lever for looking built in a tee.",
            whenToUse = "Default pick. The shoulder exercise you should never skip.",
            pickBias = 1.5),
        ExerciseDef("mwm-upright-row", "Upright Row", MuscleGroup.SHOULDERS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.INTERMEDIATE, 3, "10-12", "Pull the bar up to your collarbone",
            muscleTarget = "Side delts + traps",
            why = "MWM-989 low pulley with a straight bar. Pull it up the front of your body to your collarbone — hits the side delts and traps.",
            whenToUse = "A heavier compound option for the shoulders alongside laterals."),
        ExerciseDef("db-overhead-press", "DB Overhead Press", MuscleGroup.SHOULDERS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 3, "8-12", "Press overhead, ribs down",
            muscleTarget = "Front delts + side delts",
            why = "Builds strength and overall shoulder size. Compound movement, lots of muscle worked.",
            whenToUse = "You want to focus on shoulder STRENGTH, not just size. Or to add variety."),
        ExerciseDef("db-front-raise", "DB Front Raise", MuscleGroup.SHOULDERS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "12-15", "Front delt, controlled",
            muscleTarget = "Front delts",
            why = "Targets the front of the shoulder directly — raise the DBs straight out in front to shoulder height, controlled.",
            whenToUse = "Front delts lag, or for variety alongside laterals.",
            pickBias = 0.35),
        ExerciseDef("mwm-front-delt-raise", "Front Delt Raise", MuscleGroup.SHOULDERS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Raise the handle to shoulder height",
            muscleTarget = "Front delts",
            why = "MWM-989 low pulley. Raise the handle straight out in front to shoulder height — targets the front of the shoulder with constant tension.",
            whenToUse = "Front delts lag, or for variety alongside laterals.",
            pickBias = 0.35),

        // ── Rear delts ─────────────────────────────────────────────────────────────
        ExerciseDef("db-rear-delt-fly", "Rear Delt DB Fly", MuscleGroup.REAR_DELTS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "12-15", "Bent over, raise like wings",
            muscleTarget = "Back of shoulders",
            why = "Bend over at the waist, raise dumbbells out to the sides like wings. Builds the rear shoulder and fixes posture.",
            whenToUse = "Default rear-delt and posture work."),

        // ── Biceps ──────────────────────────────────────────────────────────────────
        ExerciseDef("db-hammer-curl", "DB Hammer Curl", MuscleGroup.BICEPS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "10-12", "Bicep + forearm",
            muscleTarget = "Bicep + forearm (palms facing each other)",
            why = "Builds the bicep AND forearm. Forearms make your arm look thicker in short sleeves.",
            whenToUse = "Default pick. Works two muscles for the price of one."),
        ExerciseDef("db-curl", "DB Curl", MuscleGroup.BICEPS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "10-12", "No swinging",
            muscleTarget = "Whole bicep",
            why = "The bread-and-butter bicep builder. Palms up, curl both DBs without swinging your elbows forward.",
            whenToUse = "Straightforward bicep work when you don't want to think about it."),
        ExerciseDef("db-incline-curl", "DB Incline Curl", MuscleGroup.BICEPS,
            listOf(Equipment.DUMBBELLS, Equipment.INCLINE_BENCH), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "10-12", "Stretched bicep = growth",
            muscleTarget = "Long head of bicep (the peak)",
            why = "The stretched position at the bottom is what builds the bicep \"peak\" you see when flexing.",
            whenToUse = "You've done hammer curls for a while and want to build the bicep peak specifically."),
        ExerciseDef("mwm-seated-bicep-curl", "Seated Bicep Curl", MuscleGroup.BICEPS,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-12", "Arms braced on the pad",
            muscleTarget = "Whole bicep, strict",
            why = "The MWM-989 seated curl station. Arms braced on the pad so it's all bicep — no swinging.",
            whenToUse = "You catch yourself swinging the weight on regular curls."),
        ExerciseDef("db-concentration-curl", "DB Concentration Curl", MuscleGroup.BICEPS,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "12-15", "Elbow braced on inner thigh",
            muscleTarget = "Bicep peak, one arm at a time",
            why = "Sit on the bench, elbow braced on inner thigh, curl one arm. Classic bodybuilder finisher.",
            whenToUse = "End-of-workout finisher, or to fix arm size imbalance.",
            pickBias = 0.7),
        ExerciseDef("mwm-standing-bicep-curl", "Standing Bicep Curl", MuscleGroup.BICEPS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-12", "Constant tension",
            muscleTarget = "Whole bicep, constant tension",
            why = "MWM-989 low pulley with a bar. Curl standing — constant tension the whole movement.",
            whenToUse = "Variety. Often grows arms faster after months of just DB curls."),

        // ── Triceps ───────────────────────────────────────────────────────────────
        ExerciseDef("db-overhead-tricep-ext", "DB Overhead Tricep Ext.", MuscleGroup.TRICEPS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "10-12", "Long head — biggest visual lever",
            muscleTarget = "Long head of tricep (the back of your arm)",
            why = "The long head is the biggest part of the tricep and gives the back of your arm visible size.",
            whenToUse = "Default pick. Biggest bang-for-buck tricep exercise."),
        ExerciseDef("db-skull-crusher", "DB Skull Crusher", MuscleGroup.TRICEPS,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.INTERMEDIATE, 3, "10-12", "Tricep mass",
            muscleTarget = "Whole tricep, mass builder",
            why = "Hits all three tricep heads. Builds raw tricep size.",
            whenToUse = "You want to load triceps heavier than overhead extensions allow."),
        ExerciseDef("mwm-tricep-pushdown", "Tricep Pushdown", MuscleGroup.TRICEPS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "MWM-989 high pulley",
            muscleTarget = "Lateral head of tricep (the outer part)",
            why = "MWM-989 high pulley with a bar. Push straight down to lock out — builds the outer tricep, most visible from the side.",
            whenToUse = "Default machine tricep exercise."),
        ExerciseDef("close-grip-db-press", "Close-Grip DB Press", MuscleGroup.TRICEPS,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "10-12", "Hands close together",
            muscleTarget = "Triceps + chest",
            why = "DB Bench Press with hands closer together. Compound — hits chest and triceps together.",
            whenToUse = "You want strength + size in one move. Good for short workouts."),
        ExerciseDef("bench-dip", "Bench Dip", MuscleGroup.TRICEPS,
            listOf(Equipment.BENCH), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.BEGINNER, 3, "AMRAP", "Hands on the bench behind you",
            muscleTarget = "Triceps (bodyweight)",
            why = "Hands on the bench behind you, slide your hips down and press back up. Loads the triceps with just your bodyweight.",
            whenToUse = "No DBs free, or as a tricep finisher.",
            pickBias = 0.6),
        ExerciseDef("diamond-push-up", "Diamond Push-Up", MuscleGroup.TRICEPS,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.INTERMEDIATE, 3, "AMRAP", "Hands in a diamond shape",
            muscleTarget = "Triceps + inner chest",
            why = "Push-up with hands close together in a diamond shape. No equipment needed.",
            whenToUse = "Tricep finisher, or no equipment available.",
            fallbackOnly = true),

        // ── Quads ─────────────────────────────────────────────────────────────────
        ExerciseDef("goblet-squat", "Goblet Squat", MuscleGroup.QUADS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 4, "10-12", "Heaviest DB you have",
            muscleTarget = "Quads (front of thigh) + glutes",
            why = "Easiest squat variation to learn. Holding the weight in front forces a good upright posture.",
            whenToUse = "Default pick until you have heavier DBs."),
        ExerciseDef("db-squat", "DB Squat", MuscleGroup.QUADS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 4, "8-12", "DBs at your sides, sit deep",
            muscleTarget = "Quads + glutes, heavier than goblet",
            why = "A dumbbell in each hand doubles the load a goblet squat allows — the natural progression once one DB stops being heavy enough.",
            whenToUse = "When goblet squats outgrow your heaviest single dumbbell."),
        ExerciseDef("db-bulgarian-split-squat", "DB Bulgarian Split Squat", MuscleGroup.QUADS,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.ADVANCED, 4, "8-10/leg", "Brutal but it works",
            muscleTarget = "Quads + glutes, one leg at a time",
            why = "Builds huge legs with light weight because all the load is on one leg. Fixes imbalances.",
            whenToUse = "Light DBs only — works great. Hard to balance at first, give it 2-3 sessions."),
        ExerciseDef("db-reverse-lunge", "DB Reverse Lunge", MuscleGroup.QUADS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "10/leg", "Step back, knee to the floor",
            muscleTarget = "Quads + glutes, one leg, knee-friendly",
            why = "Step backward into a lunge — easier on the knees than forward lunges and simpler to balance than Bulgarians.",
            whenToUse = "Knees feel cranky on squats, or you want unilateral work that's easy to learn."),
        ExerciseDef("db-step-up", "DB Step-Up", MuscleGroup.QUADS,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "10/leg", "Step onto the bench",
            muscleTarget = "Quads + glutes, unilateral",
            why = "Step up onto your bench with DBs. Simple, scalable, great for beginners.",
            whenToUse = "Beginner-friendly unilateral work. Or as a warm-up."),
        ExerciseDef("leg-extension", "Leg Extension", MuscleGroup.QUADS,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "MWM-989 leg developer",
            muscleTarget = "Quads only (pure isolation)",
            why = "MWM-989 leg developer. Hits quads without any glute, back, or balance involvement.",
            whenToUse = "After squats to fully fatigue the quads. Or when you don't want to load your spine."),
        ExerciseDef("mwm-inner-thigh", "Inner Thigh Leg Cross", MuscleGroup.QUADS,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Squeeze the legs together",
            muscleTarget = "Inner thigh (adductors)",
            why = "MWM-989 hip station. Squeeze the legs together against resistance — trains the inner-thigh adductors most leg work misses.",
            whenToUse = "Round out leg day with inner-thigh work.",
            pickBias = 0.35),

        // ── Hamstrings ──────────────────────────────────────────────────────────────
        ExerciseDef("db-romanian-deadlift", "DB Romanian Deadlift", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 4, "8-10", "Posture work too",
            muscleTarget = "Hamstrings + glutes + spinal erectors (low back)",
            why = "Best all-around posterior chain builder. Also fixes posture and builds the \"athletic back\" look.",
            whenToUse = "Default pick. Do these."),
        ExerciseDef("db-stiff-leg-deadlift", "DB Stiff-Leg Deadlift", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 3, "10-12", "Hamstring stretch",
            muscleTarget = "Hamstrings (max stretch)",
            why = "Almost-straight legs put more stretch on the hamstrings, which is what builds size.",
            whenToUse = "Variation on RDLs. More hamstring, less glute and back."),
        ExerciseDef("leg-curl", "Leg Curl", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "MWM-989 leg developer",
            muscleTarget = "Hamstrings only",
            why = "MWM-989 leg developer. Pure hamstring isolation — no spinal load, no balance.",
            whenToUse = "Low back is tired or sore. Or to finish hamstrings after RDLs."),
        ExerciseDef("db-single-leg-rdl", "Single-Leg DB RDL", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.ADVANCED, 3, "10/leg", "Hinge on one leg, balance",
            muscleTarget = "Hamstrings + glutes, one leg + balance",
            why = "Hold one DB, hinge on one leg while the other extends straight back. Builds insane balance.",
            whenToUse = "You've mastered regular RDLs and want a new challenge."),

        // ── Glutes ────────────────────────────────────────────────────────────────
        ExerciseDef("db-walking-lunge", "DB Walking Lunge", MuscleGroup.GLUTES,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "10/leg", "Unilateral balance",
            muscleTarget = "Glutes + quads, moving",
            why = "The walking pattern recruits glutes more than stationary leg exercises.",
            whenToUse = "Default pick. Easy to learn, hits glutes well."),
        // (DB Hip Thrust removed 2026-06-10: Antho's flat bench is a bench-press bench — the rack
        //  makes bracing your upper back against it for thrusts impossible.)
        ExerciseDef("db-glute-bridge", "DB Glute Bridge", MuscleGroup.GLUTES,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "12-15", "On the floor, DB on hips",
            muscleTarget = "Glutes + hamstrings",
            why = "Lie on floor, DB on hips, drive hips up. Easy on the back, hits glutes and hamstrings together.",
            whenToUse = "Low back is sore. Or as a finisher.",
            pickBias = 0.7),
        ExerciseDef("mwm-leg-kickback", "Leg Kickback", MuscleGroup.GLUTES,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Kick one leg straight back",
            muscleTarget = "Glutes only (isolation)",
            why = "MWM-989 hip station. Kick one leg straight back against resistance — pure glute isolation.",
            whenToUse = "Default machine glute exercise."),
        ExerciseDef("mwm-outer-leg-kick", "Outer Leg Kick", MuscleGroup.GLUTES,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Push the leg out to the side",
            muscleTarget = "Glutes + hip abductors (side of the hip)",
            why = "MWM-989 hip station. Push the leg out to the side against resistance — hits the glute medius on the side of the hip.",
            whenToUse = "Hip/glute shaping alongside kickbacks.",
            pickBias = 0.5),

        // ── Calves ────────────────────────────────────────────────────────────────
        ExerciseDef("standing-calf-raise", "Standing Calf Raise", MuscleGroup.CALVES,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 4, "12-15", "DB in hand",
            muscleTarget = "Upper calf (gastrocnemius — the diamond shape)",
            why = "Standing position with straight legs targets the bigger, more visible part of the calf.",
            whenToUse = "Default pick."),
        ExerciseDef("seated-calf-raise", "Seated Calf Raise", MuscleGroup.CALVES,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 4, "12-15", "Different head",
            muscleTarget = "Lower calf (soleus — the flat muscle underneath)",
            why = "Bent-knee position targets a DIFFERENT calf muscle than standing raises.",
            whenToUse = "Train both standing and seated for full calf development."),
        ExerciseDef("single-leg-calf-raise", "Single-Leg Calf Raise", MuscleGroup.CALVES,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "15-20", "Bodyweight, one leg at a time",
            muscleTarget = "One calf at a time",
            why = "No equipment needed. Bodyweight is plenty for calves.",
            whenToUse = "No equipment, or to fix imbalance between calves.",
            fallbackOnly = true),

        // ── Core ──────────────────────────────────────────────────────────────────
        ExerciseDef("hanging-knee-raise", "Hanging Knee Raise", MuscleGroup.CORE,
            listOf(Equipment.PULL_UP_BAR), ExerciseUnit.BODYWEIGHT,
            listOf(BW), Difficulty.INTERMEDIATE, 3, "10-15", "Or plank 30-60s",
            muscleTarget = "Lower abs + grip strength",
            why = "Forces lower abs to do the work. Most ab exercises miss the lower part.",
            whenToUse = "Default pick if you have a pull-up bar."),
        ExerciseDef("plank", "Plank", MuscleGroup.CORE,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "30-60s", "Brace hard, flat back",
            muscleTarget = "Whole core, isometric",
            why = "Builds the bracing strength that protects your back during squats and deadlifts.",
            whenToUse = "No pull-up bar, or as a warm-up to other core work.",
            pickBias = 0.7),
        ExerciseDef("mwm-high-pulley-crunch", "High Pulley Ab Crunch", MuscleGroup.CORE,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-15", "Kneel and crunch down",
            muscleTarget = "Upper abs (six-pack), loadable",
            why = "MWM-989 high pulley — kneel facing the machine and crunch down. Lets you ADD WEIGHT to ab work for real progression.",
            whenToUse = "You want six-pack development with actual weight progression."),
        ExerciseDef("mwm-oblique-side-bend", "Oblique Side Bend", MuscleGroup.CORE,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Bend to the side, one side at a time",
            muscleTarget = "Obliques (the sides of your waist)",
            why = "MWM-989 low pulley with a handle. Bend to the side against resistance — trains the obliques along the sides of your waist.",
            whenToUse = "Add side-ab work for a complete core."),
        ExerciseDef("lying-leg-raise", "Lying Leg Raise", MuscleGroup.CORE,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "12-15", "Lower slowly, don't arch",
            muscleTarget = "Lower abs",
            why = "Lie on the floor, raise straight legs to vertical, lower slowly without touching the ground.",
            whenToUse = "No pull-up bar available.",
            pickBias = 0.7),

        // ── Bodyweight fallbacks ─────────────────────────────────────────────────
        // Every muscle that otherwise needs equipment gets a bodyweight option here, so a
        // bodyweight-only (or minimal-equipment) setup can never produce an empty/starved day.
        // fallbackOnly = true keeps these out of an equipped user's picks — they only enter the
        // pool when nothing the user actually owns can fill the muscle.
        ExerciseDef("bw-inverted-row", "Inverted Row", MuscleGroup.BACK,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.INTERMEDIATE, 4, "AMRAP", "Under a sturdy table, body straight",
            muscleTarget = "Whole back + biceps",
            why = "Lie under a sturdy table (or low bar), grip the edge, pull your chest up keeping the body straight. The best equipment-free horizontal pull.",
            whenToUse = "Bodyweight only — your main back builder when you have no bar or machine.",
            fallbackOnly = true),
        ExerciseDef("bw-superman", "Superman", MuscleGroup.BACK,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "12-15", "Lift chest + legs off the floor",
            muscleTarget = "Lower/mid back + erectors",
            why = "Lie face-down, lift your chest and legs off the floor and squeeze. Hits the back chain with zero equipment.",
            whenToUse = "Bodyweight back work alongside inverted rows.",
            fallbackOnly = true),
        ExerciseDef("bw-pike-push-up", "Pike Push-Up", MuscleGroup.SHOULDERS,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.INTERMEDIATE, 3, "AMRAP", "Hips high, press head toward floor",
            muscleTarget = "Front + side delts",
            why = "Hips up in an inverted-V, press your head toward the floor — a bodyweight overhead press for the shoulders.",
            whenToUse = "Bodyweight only — your pressing shoulder builder.",
            fallbackOnly = true),
        ExerciseDef("bw-prone-reverse-fly", "Prone Reverse Fly", MuscleGroup.REAR_DELTS,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "15-20", "Face-down, raise arms like wings",
            muscleTarget = "Rear delts + upper back",
            why = "Lie face-down, raise straight arms out to the sides like wings and squeeze. Bodyweight rear-delt and posture work.",
            whenToUse = "Bodyweight only, or as posture work anywhere.",
            fallbackOnly = true),
        ExerciseDef("bw-doorframe-curl", "Doorframe Curl", MuscleGroup.BICEPS,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.INTERMEDIATE, 3, "10-15", "Lean back, curl your bodyweight up",
            muscleTarget = "Biceps",
            why = "Grip a sturdy doorframe edge (or towel around a post), lean back and curl your bodyweight up. The only real equipment-free biceps move.",
            whenToUse = "Bodyweight only — when you have no dumbbells or bar.",
            fallbackOnly = true),
        ExerciseDef("bw-squat", "Bodyweight Squat", MuscleGroup.QUADS,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.BEGINNER, 4, "15-20", "Full depth, controlled",
            muscleTarget = "Quads + glutes",
            why = "The foundation lower-body movement. Sit back to full depth and drive up. Add a pause or slow tempo to make it harder.",
            whenToUse = "Bodyweight only — your main squat pattern.",
            fallbackOnly = true),
        ExerciseDef("bw-reverse-lunge", "Bodyweight Reverse Lunge", MuscleGroup.QUADS,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.BEGINNER, 3, "12/leg", "Step back, knee toward floor",
            muscleTarget = "Quads + glutes, one leg",
            why = "Step backward into a lunge — knee-friendly unilateral leg work with no equipment.",
            whenToUse = "Bodyweight only, or to add single-leg work.",
            fallbackOnly = true),
        ExerciseDef("bw-good-morning", "Bodyweight Good Morning", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.BEGINNER, 3, "15-20", "Hands behind head, hinge at the hips",
            muscleTarget = "Hamstrings + glutes + lower back",
            why = "Hands behind your head, soft knees, hinge forward at the hips and feel the hamstrings stretch, then stand tall. A bodyweight posterior-chain hinge.",
            whenToUse = "Bodyweight only — your hamstring hinge.",
            fallbackOnly = true),
        ExerciseDef("bw-sliding-leg-curl", "Sliding Leg Curl", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.INTERMEDIATE, 3, "10-15", "Heels on a towel, slide out and curl back",
            muscleTarget = "Hamstrings",
            why = "Bridge up with your heels on a towel/socks on a smooth floor, slide your feet out, then curl them back in. Bodyweight hamstring isolation.",
            whenToUse = "Bodyweight only, or to finish hamstrings.",
            fallbackOnly = true),
        ExerciseDef("bw-single-leg-glute-bridge", "Single-Leg Glute Bridge", MuscleGroup.GLUTES,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "12-15/leg", "One foot down, drive hips up",
            muscleTarget = "Glutes + hamstrings, one side",
            why = "Lie on your back, one foot planted, drive your hips up on a single leg. Loads the glute hard with just bodyweight.",
            whenToUse = "Bodyweight only — your glute builder.",
            fallbackOnly = true),

        // ═══ Generalized library (2026-06-11) — for users beyond the owner's MWM-989 home gym ═══
        // Barbell / Smith / trap-bar / EZ-bar / kettlebell / band movements + generic (non-MWM)
        // machine & cable work. All read in lb (ExerciseUnit.WEIGHT, not plate-count), so none are
        // curatedOnly — they fill ordinary equipment-filtered pools that the owner's frozen preset
        // never draws from.

        // ── Chest (generalized) ──
        ExerciseDef("barbell-bench-press", "Barbell Bench Press", MuscleGroup.CHEST,
            listOf(Equipment.BARBELL, Equipment.BENCH), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 3, "5-8", "Shoulder blades pinched, bar to mid-chest",
            muscleTarget = "Whole chest — the heaviest pressing option",
            why = "The most loadable chest builder. A barbell lets you add weight in small jumps for years.",
            whenToUse = "Default heavy chest lift if you have a barbell and a rack or bench with uprights."),
        ExerciseDef("incline-barbell-bench", "Incline Barbell Bench", MuscleGroup.CHEST,
            listOf(Equipment.BARBELL, Equipment.INCLINE_BENCH), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 3, "6-10", "Bench at ~30°, bar to upper chest",
            muscleTarget = "Upper chest",
            why = "Loads the upper chest heavier than dumbbells — fills out the top of the chest.",
            whenToUse = "Upper chest lags and you want to push real weight."),
        ExerciseDef("machine-chest-press", "Machine Chest Press", MuscleGroup.CHEST,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "8-12", "Handles at mid-chest height",
            muscleTarget = "Whole chest, fixed path",
            why = "The machine guides the path so you can push hard without balancing the load — push to failure safely.",
            whenToUse = "Your main machine chest press at a commercial gym."),
        ExerciseDef("pec-deck", "Pec Deck", MuscleGroup.CHEST,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Squeeze the pads together, slow return",
            muscleTarget = "Chest only (squeeze)",
            why = "Isolates the chest with constant tension and no triceps — a clean finisher after pressing.",
            whenToUse = "Second chest movement after a press."),
        ExerciseDef("cable-fly", "Cable Fly", MuscleGroup.CHEST,
            listOf(Equipment.CABLE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Slight elbow bend, hug it out then in",
            muscleTarget = "Chest only (stretch + squeeze)",
            why = "Cables keep tension on the chest through the whole range — stretch at the bottom, squeeze at the top.",
            whenToUse = "Chest isolation when you have a cable stack."),
        ExerciseDef("smith-bench-press", "Smith Bench Press", MuscleGroup.CHEST,
            listOf(Equipment.SMITH_MACHINE, Equipment.BENCH), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "6-10", "Fixed bar path, control the descent",
            muscleTarget = "Whole chest, guided bar",
            why = "The fixed bar path lets you press heavy without a spotter — a safe way to overload the chest.",
            whenToUse = "Heavy pressing when training alone."),

        ExerciseDef("dip", "Dip", MuscleGroup.CHEST,
            listOf(Equipment.DIP_STATION), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.INTERMEDIATE, 3, "6-10", "Lean forward, elbows ~45°, deep but pain-free",
            muscleTarget = "Lower chest + triceps + front delts",
            why = "The bodyweight bench press. Leaning forward shifts the load onto the chest, and a backpack adds weight when reps get easy.",
            whenToUse = "Heavy pressing on dip bars, no bench needed."),

        // ── Back (generalized) ──
        ExerciseDef("barbell-row", "Barbell Row", MuscleGroup.BACK,
            listOf(Equipment.BARBELL), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 4, "6-10", "Hinge ~45°, pull the bar to your lower ribs",
            muscleTarget = "Whole back — thickness",
            why = "The heaviest horizontal pull. Builds back thickness and grip, and loads easily over time.",
            whenToUse = "Default heavy back builder if you have a barbell."),
        ExerciseDef("lat-pulldown", "Lat Pulldown", MuscleGroup.BACK,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.BEGINNER, 4, "8-12", "Wide grip, pull to the upper chest",
            muscleTarget = "Lats — back width",
            why = "Builds the V-taper and is easier to learn than pull-ups. Set any weight to hit any rep range.",
            whenToUse = "Your main machine back-width exercise."),
        ExerciseDef("seated-cable-row", "Seated Cable Row", MuscleGroup.BACK,
            listOf(Equipment.CABLE), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "10-12", "Chest tall, pull to your stomach, squeeze",
            muscleTarget = "Mid-back thickness",
            why = "Constant cable tension builds the middle back with a strong squeeze and no lower-back strain.",
            whenToUse = "Horizontal pull when you have a cable row station."),
        ExerciseDef("suspension-row", "Suspension Row", MuscleGroup.BACK,
            listOf(Equipment.SUSPENSION), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.BEGINNER, 3, "8-12", "Body straight, pull your chest to the handles",
            muscleTarget = "Whole back + biceps",
            why = "A row you load by walking your feet forward: one step turns it from easy to brutal, no weights needed.",
            whenToUse = "Horizontal pulling on straps or rings."),
        ExerciseDef("band-row", "Band Row", MuscleGroup.BACK,
            listOf(Equipment.RESISTANCE_BAND), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.BEGINNER, 3, "12-15", "Band around a post, squeeze the shoulder blades",
            muscleTarget = "Whole back + biceps",
            why = "Anchor a band and row. Resistance peaks at the squeeze, right where your back works hardest.",
            whenToUse = "Back work when all you have is a band.",
            fallbackOnly = true),

        // ── Shoulders (generalized) ──
        ExerciseDef("barbell-overhead-press", "Barbell Overhead Press", MuscleGroup.SHOULDERS,
            listOf(Equipment.BARBELL), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 3, "5-8", "Brace hard, press overhead, ribs down",
            muscleTarget = "Front + side delts, strength",
            why = "The strongest pressing movement for the shoulders — builds size and overhead strength.",
            whenToUse = "Heavy shoulder day lead if you have a barbell."),
        ExerciseDef("machine-shoulder-press", "Machine Shoulder Press", MuscleGroup.SHOULDERS,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "8-12", "Press to nearly locked, control down",
            muscleTarget = "Front + side delts",
            why = "Guided overhead press — push the shoulders hard without balancing dumbbells.",
            whenToUse = "Machine alternative to the overhead press."),
        ExerciseDef("cable-lateral-raise", "Cable Lateral Raise", MuscleGroup.SHOULDERS,
            listOf(Equipment.CABLE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Cable behind you, raise to shoulder height",
            muscleTarget = "Side delts — width, constant tension",
            why = "A cable keeps tension on the side delt at the bottom where dumbbells go light — often grows width faster.",
            whenToUse = "Side-delt work when you have a low cable.",
            pickBias = 1.2),
        ExerciseDef("smith-shoulder-press", "Smith Shoulder Press", MuscleGroup.SHOULDERS,
            listOf(Equipment.SMITH_MACHINE), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "6-10", "Seated, fixed path overhead",
            muscleTarget = "Front + side delts, guided",
            why = "The fixed path lets you overload the press safely with no balancing.",
            whenToUse = "Heavy overhead pressing when training alone.",
            pickBias = 0.7),
        ExerciseDef("kb-press", "KB Overhead Press", MuscleGroup.SHOULDERS,
            listOf(Equipment.KETTLEBELL), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 3, "6-10", "Bell in the rack position, press overhead",
            muscleTarget = "Front + side delts",
            why = "The kettlebell's offset weight makes the shoulder and core work harder to stabilize each press.",
            whenToUse = "Overhead pressing with a kettlebell.",
            pickBias = 0.8),
        ExerciseDef("band-lateral-raise", "Band Lateral Raise", MuscleGroup.SHOULDERS,
            listOf(Equipment.RESISTANCE_BAND), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "15-20", "Stand on the band, raise to shoulder height",
            muscleTarget = "Side delts",
            why = "Stand on a band and raise out to the sides — resistance rises at the top where the side delt works hardest.",
            whenToUse = "Side-delt work when all you have is a band.",
            fallbackOnly = true),

        // ── Rear delts (generalized) ──
        ExerciseDef("face-pull", "Face Pull", MuscleGroup.REAR_DELTS,
            listOf(Equipment.CABLE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "15-20", "Rope to your forehead, elbows high",
            muscleTarget = "Rear delts + upper back, posture",
            why = "The best rear-delt and posture exercise — pulls the shoulders back and balances all that pressing.",
            whenToUse = "Default rear-delt work when you have a cable.",
            pickBias = 1.2),
        ExerciseDef("reverse-pec-deck", "Reverse Pec Deck", MuscleGroup.REAR_DELTS,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Arms out like wings, squeeze the rear delts",
            muscleTarget = "Rear delts",
            why = "Isolates the rear delts with a guided path — easy to feel and progress.",
            whenToUse = "Rear-delt isolation at a commercial gym."),
        ExerciseDef("band-pull-apart", "Band Pull-Apart", MuscleGroup.REAR_DELTS,
            listOf(Equipment.RESISTANCE_BAND), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "15-20", "Arms straight, pull the band apart to your chest",
            muscleTarget = "Rear delts + upper back",
            why = "Hold a band at arm's length and pull it apart. Endless rear-delt and posture volume with minimal gear.",
            whenToUse = "Rear-delt and posture work when all you have is a band."),
        ExerciseDef("suspension-face-pull", "Suspension Face Pull", MuscleGroup.REAR_DELTS,
            listOf(Equipment.SUSPENSION), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "12-15", "Lean back, pull the handles to your temples",
            muscleTarget = "Rear delts + upper back, posture",
            why = "The cable face pull on straps: lean back and pull to your face, elbows high. Steeper lean, harder set.",
            whenToUse = "Rear-delt work on a suspension trainer, no cable needed.",
            pickBias = 0.9),

        // ── Biceps (generalized) ──
        ExerciseDef("barbell-curl", "Barbell Curl", MuscleGroup.BICEPS,
            listOf(Equipment.BARBELL), ExerciseUnit.WEIGHT,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "8-12", "Elbows pinned, no swinging",
            muscleTarget = "Whole bicep — heaviest curl",
            why = "Lets you load both arms heavier than dumbbells — the classic mass builder for the biceps.",
            whenToUse = "Default heavy curl if you have a barbell."),
        ExerciseDef("ez-bar-curl", "EZ-Bar Curl", MuscleGroup.BICEPS,
            listOf(Equipment.EZ_BAR), ExerciseUnit.WEIGHT,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "8-12", "Angled grip, elbows still",
            muscleTarget = "Whole bicep, wrist-friendly",
            why = "The angled bar is easier on the wrists than a straight bar while still loading heavy.",
            whenToUse = "Heavy curls when a straight bar bothers your wrists."),
        ExerciseDef("cable-curl", "Cable Curl", MuscleGroup.BICEPS,
            listOf(Equipment.CABLE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-15", "Constant tension top to bottom",
            muscleTarget = "Whole bicep, constant tension",
            why = "A cable keeps the bicep loaded the whole rep — great pump and easy to progress.",
            whenToUse = "Bicep work when you have a low cable."),
        ExerciseDef("preacher-curl", "Preacher Curl", MuscleGroup.BICEPS,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-12", "Arms braced on the pad, full range",
            muscleTarget = "Lower bicep, strict",
            why = "Bracing your arms on the pad removes all swing — pure bicep, with a strong stretch at the bottom.",
            whenToUse = "When you catch yourself swinging on standing curls."),

        // ── Triceps (generalized) ──
        ExerciseDef("close-grip-bench-press", "Close-Grip Bench Press", MuscleGroup.TRICEPS,
            listOf(Equipment.BARBELL, Equipment.BENCH), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 3, "6-10", "Hands shoulder-width, elbows tucked",
            muscleTarget = "Triceps + chest",
            why = "The heaviest triceps movement — a barbell press with a close grip that overloads the triceps.",
            whenToUse = "When you want to load triceps heavy alongside some chest."),
        ExerciseDef("cable-tricep-pushdown", "Cable Tricep Pushdown", MuscleGroup.TRICEPS,
            listOf(Equipment.CABLE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-15", "Elbows pinned, push to lockout",
            muscleTarget = "Triceps (outer head)",
            why = "The go-to triceps isolation — constant cable tension, easy to progress, easy on the elbows.",
            whenToUse = "Default triceps work when you have a cable."),
        ExerciseDef("ez-bar-skullcrusher", "EZ-Bar Skullcrusher", MuscleGroup.TRICEPS,
            listOf(Equipment.EZ_BAR, Equipment.BENCH), ExerciseUnit.WEIGHT,
            listOf(ISO, FW), Difficulty.INTERMEDIATE, 3, "8-12", "Lower the bar to your forehead, elbows still",
            muscleTarget = "Whole triceps — mass",
            why = "Hits all three triceps heads under a loaded stretch — a top mass builder for the arms.",
            whenToUse = "Heavier triceps work after pressing."),
        ExerciseDef("dip-machine", "Triceps Dip (machine)", MuscleGroup.TRICEPS,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "8-12", "Press down to lockout, control up",
            muscleTarget = "Triceps + lower chest",
            why = "A guided dip — loads the triceps like a press without needing to support your full bodyweight.",
            whenToUse = "Machine triceps press at a commercial gym."),

        // ── Quads (generalized) ──
        ExerciseDef("back-squat", "Back Squat", MuscleGroup.QUADS,
            listOf(Equipment.BARBELL, Equipment.SQUAT_RACK), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 4, "5-8", "Brace, sit between your hips, drive up",
            muscleTarget = "Quads + glutes — the king of leg lifts",
            why = "The most loadable lower-body movement. Builds the whole lower body and progresses for years.",
            whenToUse = "Default heavy leg lift if you have a barbell and rack.",
            pickBias = 1.3),
        ExerciseDef("front-squat", "Front Squat", MuscleGroup.QUADS,
            listOf(Equipment.BARBELL, Equipment.SQUAT_RACK), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.ADVANCED, 3, "6-10", "Bar on the front delts, elbows high, stay upright",
            muscleTarget = "Quads (more than back squat)",
            why = "The bar in front forces an upright torso, shifting the work onto the quads and sparing the lower back.",
            whenToUse = "Quad emphasis, or when back squats bother your lower back.",
            pickBias = 0.8),
        ExerciseDef("leg-press", "Leg Press", MuscleGroup.QUADS,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "10-15", "Feet mid-platform, knees track over toes",
            muscleTarget = "Quads + glutes, no balance needed",
            why = "Loads the legs heavy with zero balance or spinal load — great for pushing close to failure safely.",
            whenToUse = "Your main machine leg builder.",
            pickBias = 1.1),
        ExerciseDef("hack-squat", "Hack Squat", MuscleGroup.QUADS,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.INTERMEDIATE, 3, "8-12", "Back on the pad, deep but controlled",
            muscleTarget = "Quads (sweep)",
            why = "A guided squat that hammers the quads — especially the outer sweep — without balancing a bar.",
            whenToUse = "Quad-focused machine squat."),
        ExerciseDef("smith-squat", "Smith Squat", MuscleGroup.QUADS,
            listOf(Equipment.SMITH_MACHINE), ExerciseUnit.WEIGHT,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "8-12", "Feet slightly forward, fixed path",
            muscleTarget = "Quads + glutes, guided",
            why = "The fixed path lets you squat heavy and chase the quads with no balance demand.",
            whenToUse = "Squatting heavy when training alone.",
            pickBias = 0.8),
        ExerciseDef("kb-goblet-squat", "KB Goblet Squat", MuscleGroup.QUADS,
            listOf(Equipment.KETTLEBELL), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "10-15", "Hold the bell at your chest, sit deep",
            muscleTarget = "Quads + glutes",
            why = "A kettlebell at your chest keeps you upright and is the easiest squat to learn.",
            whenToUse = "Squat pattern when a kettlebell is what you've got."),
        ExerciseDef("pause-squat", "Pause Squat", MuscleGroup.QUADS,
            listOf(Equipment.BARBELL, Equipment.SQUAT_RACK), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.ADVANCED, 3, "3-6", "2-second pause at the bottom, then drive",
            muscleTarget = "Quads — strength out of the hole",
            why = "Pausing at the bottom kills the bounce and builds strength and control where squats are hardest.",
            whenToUse = "Strength block, or to fix a weak bottom position.",
            pickBias = 0.5),
        ExerciseDef("trap-bar-deadlift", "Trap-Bar Deadlift", MuscleGroup.QUADS,
            listOf(Equipment.TRAP_BAR), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "5-8", "Stand inside the bar, push the floor away",
            muscleTarget = "Quads + glutes + back, beginner-friendly",
            why = "Standing inside the bar keeps you more upright than a barbell deadlift — easier on the back, great for the legs.",
            whenToUse = "A safer heavy pull than the conventional deadlift if you have a trap bar.",
            pickBias = 0.9),

        // ── Hamstrings (generalized) ──
        ExerciseDef("barbell-rdl", "Barbell RDL", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.BARBELL), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 4, "6-10", "Push hips back, soft knees, flat back",
            muscleTarget = "Hamstrings + glutes — heaviest hinge",
            why = "The most loadable hamstring builder. Push the hips back and feel the stretch, then stand tall.",
            whenToUse = "Default heavy posterior-chain lift if you have a barbell.",
            pickBias = 1.1),
        ExerciseDef("conventional-deadlift", "Conventional Deadlift", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.BARBELL), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.ADVANCED, 3, "3-6", "Brace, push the floor away, lock out tall",
            muscleTarget = "Whole posterior chain — hamstrings, glutes, back",
            why = "The heaviest pull there is — builds the entire backside of the body and raw strength.",
            whenToUse = "Heavy strength work when you have a barbell and know the hinge.",
            pickBias = 0.9),
        ExerciseDef("lying-leg-curl", "Lying Leg Curl", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-15", "Curl your heels to your glutes, slow return",
            muscleTarget = "Hamstrings only",
            why = "Pure hamstring isolation with no spinal load — the only move that trains the knee-bending function.",
            whenToUse = "Hamstring isolation, or when your lower back is tired."),

        // ── Glutes (generalized) ──
        ExerciseDef("barbell-hip-thrust", "Barbell Hip Thrust", MuscleGroup.GLUTES,
            listOf(Equipment.BARBELL, Equipment.BENCH), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 4, "8-12", "Upper back on the bench, drive hips to lockout",
            muscleTarget = "Glutes — heaviest glute builder",
            why = "The most loadable glute exercise — back on a bench, bar over the hips, squeeze hard at the top.",
            whenToUse = "Default heavy glute work with a barbell and bench."),
        ExerciseDef("cable-glute-kickback", "Cable Glute Kickback", MuscleGroup.GLUTES,
            listOf(Equipment.CABLE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Cuff on the ankle, kick straight back, squeeze",
            muscleTarget = "Glutes (isolation)",
            why = "Isolates one glute at a time with constant cable tension — great for shaping and mind-muscle work.",
            whenToUse = "Glute isolation when you have a low cable."),
        ExerciseDef("kb-swing", "KB Swing", MuscleGroup.GLUTES,
            listOf(Equipment.KETTLEBELL), ExerciseUnit.WEIGHT,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 3, "12-15", "Hinge and snap the hips — it's not a squat",
            muscleTarget = "Glutes + hamstrings, explosive",
            why = "An explosive hip hinge that builds the glutes and conditioning at once — power comes from snapping the hips.",
            whenToUse = "Glute and conditioning work with a kettlebell."),

        // ── Calves (generalized) ──
        ExerciseDef("calf-raise-machine", "Calf Raise (machine)", MuscleGroup.CALVES,
            listOf(Equipment.MACHINE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 4, "12-15", "Full stretch at the bottom, pause at the top",
            muscleTarget = "Calves",
            why = "Loads the calves heavier than a dumbbell can, with a big stretch — calves need both load and range.",
            whenToUse = "Calf work at a gym with a calf machine."),

        // ── Core (generalized) ──
        ExerciseDef("cable-crunch", "Cable Crunch", MuscleGroup.CORE,
            listOf(Equipment.CABLE), ExerciseUnit.WEIGHT,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Kneel, rope by your head, crunch down",
            muscleTarget = "Upper abs, loadable",
            why = "Kneel at a high cable and crunch down — lets you ADD WEIGHT to ab work for real progression.",
            whenToUse = "When you want weighted ab work with actual overload."),
        ExerciseDef("ab-wheel-rollout", "Ab Wheel Rollout", MuscleGroup.CORE,
            listOf(Equipment.AB_WHEEL), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.INTERMEDIATE, 3, "8-12", "Roll out only as far as your back stays flat",
            muscleTarget = "Whole core, anti-extension",
            why = "One of the hardest ab moves per rep: the wheel makes your core fight the stretch the whole way out.",
            whenToUse = "Core work with an ab wheel. Progress by rolling further."),
        ExerciseDef("captains-chair-knee-raise", "Captain's Chair Knee Raise", MuscleGroup.CORE,
            listOf(Equipment.DIP_STATION), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "10-15", "Forearms on the pads, knees to your chest",
            muscleTarget = "Lower abs",
            why = "Braced on the dip tower's arm pads, raise your knees. Lower-ab work with no grip limit and no bar hang.",
            whenToUse = "Lower abs on a dip station.",
            pickBias = 0.8)
    )

    private val byId: Map<String, ExerciseDef> = all.associateBy { it.id }
    private val byName: Map<String, ExerciseDef> = all.associateBy { it.name }

    fun byId(id: String): ExerciseDef? = byId[id]
    fun byName(name: String): ExerciseDef? = byName[name]

    fun forMuscle(muscle: MuscleGroup): List<ExerciseDef> = all.filter { it.muscle == muscle }

    /**
     * Movement pattern per exercise (program-unlock Phase 4). Only the compound / core movements are
     * listed; everything else is single-joint accessory work → [MovementPattern.ISOLATION] by default.
     * Kept as one compact map rather than a field on every entry — single source, easy to tweak.
     */
    private val patterns: Map<String, MovementPattern> = mapOf(
        // Pressing
        "db-bench-press" to MovementPattern.HORIZONTAL_PUSH,
        "incline-db-bench-press" to MovementPattern.HORIZONTAL_PUSH,
        "mwm-seated-bench-press" to MovementPattern.HORIZONTAL_PUSH,
        "push-up" to MovementPattern.HORIZONTAL_PUSH,
        "close-grip-db-press" to MovementPattern.HORIZONTAL_PUSH,
        "diamond-push-up" to MovementPattern.HORIZONTAL_PUSH,
        "db-overhead-press" to MovementPattern.VERTICAL_PUSH,
        "bench-dip" to MovementPattern.VERTICAL_PUSH,
        "dip" to MovementPattern.VERTICAL_PUSH,
        // Front raises are anterior-delt flexion — same prime mover as a vertical press. Classing
        // them VERTICAL_PUSH stops a day stacking OHP + front raise, or two front-raise variants.
        "db-front-raise" to MovementPattern.VERTICAL_PUSH,
        "mwm-front-delt-raise" to MovementPattern.VERTICAL_PUSH,
        // Pulling
        "mwm-wide-lat-pulldown" to MovementPattern.VERTICAL_PULL,
        "mwm-close-grip-pulldown" to MovementPattern.VERTICAL_PULL,
        "pull-up" to MovementPattern.VERTICAL_PULL,
        "chin-up" to MovementPattern.VERTICAL_PULL,
        "db-row" to MovementPattern.HORIZONTAL_PULL,
        "chest-supported-db-row" to MovementPattern.HORIZONTAL_PULL,
        // Legs
        "goblet-squat" to MovementPattern.SQUAT,
        "db-squat" to MovementPattern.SQUAT,
        "db-bulgarian-split-squat" to MovementPattern.LUNGE,
        "db-reverse-lunge" to MovementPattern.LUNGE,
        "db-step-up" to MovementPattern.LUNGE,
        "db-walking-lunge" to MovementPattern.LUNGE,
        "db-romanian-deadlift" to MovementPattern.HINGE,
        "db-stiff-leg-deadlift" to MovementPattern.HINGE,
        "db-single-leg-rdl" to MovementPattern.HINGE,
        // Glute bridges are deliberately NOT classed HINGE: no spinal load, different
        // length-tension — RDL + bridge in one day is good programming, not a duplicate.
        // Core
        "hanging-knee-raise" to MovementPattern.CORE,
        "plank" to MovementPattern.CORE,
        "mwm-high-pulley-crunch" to MovementPattern.CORE,
        "lying-leg-raise" to MovementPattern.CORE,
        "ab-wheel-rollout" to MovementPattern.CORE,
        "captains-chair-knee-raise" to MovementPattern.CORE,
        // Bodyweight fallbacks
        "bw-inverted-row" to MovementPattern.HORIZONTAL_PULL,
        "bw-pike-push-up" to MovementPattern.VERTICAL_PUSH,
        "bw-squat" to MovementPattern.SQUAT,
        "bw-reverse-lunge" to MovementPattern.LUNGE,
        "bw-good-morning" to MovementPattern.HINGE,
        // Generalized library (2026-06-11)
        "barbell-bench-press" to MovementPattern.HORIZONTAL_PUSH,
        "incline-barbell-bench" to MovementPattern.HORIZONTAL_PUSH,
        "machine-chest-press" to MovementPattern.HORIZONTAL_PUSH,
        "smith-bench-press" to MovementPattern.HORIZONTAL_PUSH,
        "close-grip-bench-press" to MovementPattern.HORIZONTAL_PUSH,
        "barbell-overhead-press" to MovementPattern.VERTICAL_PUSH,
        "machine-shoulder-press" to MovementPattern.VERTICAL_PUSH,
        "smith-shoulder-press" to MovementPattern.VERTICAL_PUSH,
        "kb-press" to MovementPattern.VERTICAL_PUSH,
        "dip-machine" to MovementPattern.VERTICAL_PUSH,
        "lat-pulldown" to MovementPattern.VERTICAL_PULL,
        "barbell-row" to MovementPattern.HORIZONTAL_PULL,
        "seated-cable-row" to MovementPattern.HORIZONTAL_PULL,
        "suspension-row" to MovementPattern.HORIZONTAL_PULL,
        "band-row" to MovementPattern.HORIZONTAL_PULL,
        "back-squat" to MovementPattern.SQUAT,
        "front-squat" to MovementPattern.SQUAT,
        "pause-squat" to MovementPattern.SQUAT,
        "leg-press" to MovementPattern.SQUAT,
        "hack-squat" to MovementPattern.SQUAT,
        "smith-squat" to MovementPattern.SQUAT,
        "kb-goblet-squat" to MovementPattern.SQUAT,
        "barbell-rdl" to MovementPattern.HINGE,
        "conventional-deadlift" to MovementPattern.HINGE,
        "trap-bar-deadlift" to MovementPattern.HINGE,
        "kb-swing" to MovementPattern.HINGE,
        "cable-crunch" to MovementPattern.CORE
    )

    fun patternOf(def: ExerciseDef): MovementPattern =
        patterns[def.id] ?: MovementPattern.ISOLATION

    /**
     * Which problem areas each movement stresses (program-unlock Phase 3). Only listed exercises have
     * contraindications; everything else is unflagged. A movement can stress more than one area.
     */
    private val contraindications: Map<String, Set<ProblemArea>> = mapOf(
        // ── Squat pattern — knees (flexion under load), hips (deep flexion), ankles (dorsiflexion).
        //    Back-loaded barbell squats add lower back + neck (bar across the traps).
        "goblet-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "db-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "front-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "leg-press" to setOf(ProblemArea.KNEES, ProblemArea.HIPS),
        "hack-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "smith-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "kb-goblet-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "bw-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "back-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES, ProblemArea.LOWER_BACK, ProblemArea.NECK),
        "pause-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES, ProblemArea.LOWER_BACK, ProblemArea.NECK),
        // ── Lunge pattern — knees + hips, plus ankles (single-leg balance / deep dorsiflexion).
        "db-bulgarian-split-squat" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "db-reverse-lunge" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "db-walking-lunge" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "db-step-up" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        "bw-reverse-lunge" to setOf(ProblemArea.KNEES, ProblemArea.HIPS, ProblemArea.ANKLES),
        // ── Knee isolation.
        "leg-extension" to setOf(ProblemArea.KNEES),
        // ── Hip hinges — lower back + hips (hip-dominant load). Single-leg RDL adds ankles (balance).
        "db-romanian-deadlift" to setOf(ProblemArea.LOWER_BACK, ProblemArea.HIPS),
        "db-stiff-leg-deadlift" to setOf(ProblemArea.LOWER_BACK, ProblemArea.HIPS),
        "db-single-leg-rdl" to setOf(ProblemArea.LOWER_BACK, ProblemArea.HIPS, ProblemArea.ANKLES),
        "barbell-rdl" to setOf(ProblemArea.LOWER_BACK, ProblemArea.HIPS),
        "conventional-deadlift" to setOf(ProblemArea.LOWER_BACK, ProblemArea.HIPS),
        "trap-bar-deadlift" to setOf(ProblemArea.LOWER_BACK, ProblemArea.HIPS),
        "kb-swing" to setOf(ProblemArea.LOWER_BACK, ProblemArea.HIPS),
        "bw-good-morning" to setOf(ProblemArea.LOWER_BACK, ProblemArea.HIPS),
        // ── Bent-over rows — lower back.
        "db-row" to setOf(ProblemArea.LOWER_BACK),
        "barbell-row" to setOf(ProblemArea.LOWER_BACK),
        // ── Direct hip-joint loading — heavy thrust + abduction/adduction machines.
        "barbell-hip-thrust" to setOf(ProblemArea.HIPS),
        "mwm-inner-thigh" to setOf(ProblemArea.HIPS),
        "mwm-outer-leg-kick" to setOf(ProblemArea.HIPS),
        // ── Overhead / shoulder-stressing presses & raises — loaded overhead work adds neck.
        "db-overhead-press" to setOf(ProblemArea.SHOULDERS, ProblemArea.NECK),
        "barbell-overhead-press" to setOf(ProblemArea.SHOULDERS, ProblemArea.NECK),
        "machine-shoulder-press" to setOf(ProblemArea.SHOULDERS, ProblemArea.NECK),
        "smith-shoulder-press" to setOf(ProblemArea.SHOULDERS, ProblemArea.NECK),
        "kb-press" to setOf(ProblemArea.SHOULDERS, ProblemArea.NECK),
        "mwm-upright-row" to setOf(ProblemArea.SHOULDERS, ProblemArea.NECK),
        "db-fly" to setOf(ProblemArea.SHOULDERS),
        "db-pullover" to setOf(ProblemArea.SHOULDERS),
        "db-front-raise" to setOf(ProblemArea.SHOULDERS),
        "mwm-front-delt-raise" to setOf(ProblemArea.SHOULDERS),
        "pec-deck" to setOf(ProblemArea.SHOULDERS),
        "cable-fly" to setOf(ProblemArea.SHOULDERS),
        // ── Dips / close-grip presses — shoulders and/or elbows (loaded lockout).
        "bench-dip" to setOf(ProblemArea.SHOULDERS, ProblemArea.WRISTS, ProblemArea.ELBOWS),
        "dip-machine" to setOf(ProblemArea.SHOULDERS, ProblemArea.ELBOWS),
        // Full bodyweight dips load the shoulder at a deep stretch — the harshest of the dip family.
        "dip" to setOf(ProblemArea.SHOULDERS, ProblemArea.ELBOWS, ProblemArea.WRISTS),
        // The rollout is a loaded anti-extension hold — risky with a cranky lower back or shoulders.
        "ab-wheel-rollout" to setOf(ProblemArea.LOWER_BACK, ProblemArea.SHOULDERS),
        "close-grip-bench-press" to setOf(ProblemArea.ELBOWS),
        "close-grip-db-press" to setOf(ProblemArea.ELBOWS),
        // ── Elbow-stressing arm work — loaded skull crushers / overhead ext / straight-bar & preacher curls.
        //    (Gentler picks — DB/hammer/cable curls, pushdowns — stay unflagged so the plan can steer to them.)
        "db-skull-crusher" to setOf(ProblemArea.ELBOWS),
        "ez-bar-skullcrusher" to setOf(ProblemArea.ELBOWS),
        "db-overhead-tricep-ext" to setOf(ProblemArea.ELBOWS),
        "barbell-curl" to setOf(ProblemArea.ELBOWS),
        "preacher-curl" to setOf(ProblemArea.ELBOWS),
        // ── Wrist-loading (bodyweight on the hands). Diamond push-up also loads the elbows.
        "push-up" to setOf(ProblemArea.WRISTS),
        "diamond-push-up" to setOf(ProblemArea.WRISTS, ProblemArea.ELBOWS),
        "plank" to setOf(ProblemArea.WRISTS),
        "bw-pike-push-up" to setOf(ProblemArea.SHOULDERS, ProblemArea.WRISTS),
        // ── Loaded calf work — ankles (plantarflexion under a deep stretch).
        "standing-calf-raise" to setOf(ProblemArea.ANKLES),
        "seated-calf-raise" to setOf(ProblemArea.ANKLES),
        "single-leg-calf-raise" to setOf(ProblemArea.ANKLES),
        "calf-raise-machine" to setOf(ProblemArea.ANKLES)
    )

    fun contraindicationsOf(def: ExerciseDef): Set<ProblemArea> =
        contraindications[def.id] ?: emptySet()

    /** Available if no equipment is configured (empty = all) or every required item is on hand. */
    fun isAvailable(def: ExerciseDef, available: Set<Equipment>): Boolean =
        available.isEmpty() || def.equipment.all { it == Equipment.BODYWEIGHT_ONLY || it in available }

    /**
     * The exercise universe available to the user — the SINGLE source the generator, swap picker and
     * the like/dislike screen all draw from. When [frozenIds] is non-null (a curated preset such as
     * the Developer's preset) the pool is exactly those ids — locked, and immune to any movement
     * added to the library later. Otherwise it's everything [available] equipment can perform.
     */
    fun availablePool(available: Set<Equipment>, frozenIds: Set<String>?): List<ExerciseDef> =
        if (frozenIds != null) all.filter { it.id in frozenIds }
        else all.filter { !it.curatedOnly && isAvailable(it, available) }

    /**
     * Swap candidates for [muscle], drawn from the same pool the generator uses ([availablePool]):
     * excludes [disliked] exercises, honours a curated [frozenIds] preset, and otherwise filters to
     * what [available] equipment can perform.
     */
    fun swapCandidates(
        muscle: MuscleGroup,
        available: Set<Equipment>,
        disliked: Set<String>,
        frozenIds: Set<String>? = null
    ): List<ExerciseDef> =
        availablePool(available, frozenIds).filter { it.muscle == muscle && it.id !in disliked }

    /**
     * Antho's MWM-989 home gym, FROZEN (snapshot 2026-06-11): every library id his
     * {dumbbells, flat bench, cable, machine} setup could perform at that time — deliberately NO
     * incline-bench or pull-up-bar movements (his bench is flat; the bar isn't part of this preset).
     * Hardcoded, NOT computed from [isAvailable], so movements added later during the equipment
     * generalization can never leak into the curated "Developer's preset". See
     * [com.forge.app.program.equipmentPresets].
     */
    val DEVELOPER_FROZEN_IDS: Set<String> = setOf(
        // Chest
        "db-bench-press", "mwm-seated-bench-press", "db-fly", "push-up",
        // Back
        "mwm-wide-lat-pulldown", "db-row", "mwm-close-grip-pulldown", "mwm-straight-arm-pulldown", "db-pullover",
        // Shoulders
        "db-lateral-raise", "mwm-upright-row", "db-overhead-press", "db-front-raise", "mwm-front-delt-raise",
        // Rear delts
        "db-rear-delt-fly",
        // Biceps
        "db-hammer-curl", "db-curl", "mwm-seated-bicep-curl", "db-concentration-curl", "mwm-standing-bicep-curl",
        // Triceps
        "db-overhead-tricep-ext", "db-skull-crusher", "mwm-tricep-pushdown", "close-grip-db-press",
        "bench-dip", "diamond-push-up",
        // Quads
        "goblet-squat", "db-squat", "db-bulgarian-split-squat", "db-reverse-lunge", "db-step-up",
        "leg-extension", "mwm-inner-thigh",
        // Hamstrings
        "db-romanian-deadlift", "db-stiff-leg-deadlift", "leg-curl", "db-single-leg-rdl",
        // Glutes
        "db-walking-lunge", "db-glute-bridge", "mwm-leg-kickback", "mwm-outer-leg-kick",
        // Calves
        "standing-calf-raise", "seated-calf-raise", "single-leg-calf-raise",
        // Core
        "plank", "mwm-high-pulley-crunch", "mwm-oblique-side-bend", "lying-leg-raise",
        // Bodyweight fallbacks (no equipment needed — already part of this preset's swap pool today)
        "bw-inverted-row", "bw-superman", "bw-pike-push-up", "bw-prone-reverse-fly", "bw-doorframe-curl",
        "bw-squat", "bw-reverse-lunge", "bw-good-morning", "bw-sliding-leg-curl", "bw-single-leg-glute-bridge"
    )
}

/** Build a placed [ExercisePlan] from a library entry, using its default sets/reps. */
fun ExerciseDef.toPlan(): ExercisePlan = ExercisePlan(
    id = id,
    name = name,
    sets = defaultSets,
    reps = defaultReps,
    unit = unit,
    muscle = muscle,
    difficulty = difficulty,
    note = note,
    tags = tags,
    formCue = formCue,
    equipment = equipment
)
