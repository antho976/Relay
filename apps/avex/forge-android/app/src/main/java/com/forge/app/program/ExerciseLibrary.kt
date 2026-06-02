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
    val whenToUse: String? = null
)

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
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "8-10", "Fills tee neckline",
            muscleTarget = "Upper chest (the part near your collarbone)",
            why = "Builds the top of your chest, which is what fills out a t-shirt at the neckline.",
            whenToUse = "When your lower chest is catching up but the top still looks flat. Or if regular bench bores you."),
        ExerciseDef("machine-chest-press", "Machine Chest Press", MuscleGroup.CHEST,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "10-12", "MWM-989 press arm",
            muscleTarget = "Whole chest, fixed path",
            why = "Machine guides the movement, so you can't mess up the form. Easier on shoulders than free weights.",
            whenToUse = "Shoulder feels tweaky. Or your last session was so hard your stabilizer muscles are toast."),
        ExerciseDef("cable-pec-fly", "Pec Fly (cable)", MuscleGroup.CHEST,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Cross cables in front",
            muscleTarget = "Inner chest line",
            why = "Isolates the chest with no shoulder/tricep help. Cross both MWM cables in front of you. Creates the line down the middle of your chest.",
            whenToUse = "You've already pressed and want to finish the chest off without more pressing fatigue."),
        ExerciseDef("push-up", "Push-Up", MuscleGroup.CHEST,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.BEGINNER, 3, "AMRAP", "Feet elevated to make it harder",
            muscleTarget = "Whole chest + shoulders + triceps",
            why = "No equipment needed. Feet on a bench makes it harder than regular push-ups.",
            whenToUse = "Equipment is in use, or as a warm-up before pressing."),

        // ── Back ─────────────────────────────────────────────────────────────────
        ExerciseDef("lat-pulldown", "Lat Pulldown", MuscleGroup.BACK,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 4, "8-12", "Wide grip, pull to upper chest",
            muscleTarget = "Lats (the wing muscles on the sides of your back)",
            why = "Builds back WIDTH — half of the V-taper you want. Easier to learn than pull-ups.",
            whenToUse = "Default pick. The foundation back exercise."),
        ExerciseDef("close-grip-lat-pulldown", "Close-Grip Lat Pulldown", MuscleGroup.BACK,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "10-12", "Different angle",
            muscleTarget = "Lower lats + biceps assist",
            why = "Hits the bottom part of the lats more, easier on your shoulders than wide-grip.",
            whenToUse = "Shoulders feel tight on regular pulldowns. Or you want a different feel from wide-grip."),
        ExerciseDef("machine-seated-row", "Machine Seated Row", MuscleGroup.BACK,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 4, "8-12", "Mid-back thickness",
            muscleTarget = "Mid-back (between your shoulder blades)",
            why = "Builds back THICKNESS — depth from the side. Different look than pulldowns build.",
            whenToUse = "You want a \"fuller\" back look from the side, or to balance out lots of pulldowns."),
        ExerciseDef("db-row", "DB Row (1-arm)", MuscleGroup.BACK,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 4, "8-12", "Knee on bench, row to hip",
            muscleTarget = "One side of your back at a time",
            why = "Fixes left/right imbalance. Place a knee on the bench, other foot on floor, row the DB to your hip.",
            whenToUse = "You notice one side does more work than the other. Or as a finisher."),
        ExerciseDef("chest-supported-db-row", "Chest-Supported DB Row", MuscleGroup.BACK,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "10-12", "Lie chest-down on an incline bench",
            muscleTarget = "Mid-back, both sides, no lower-back strain",
            why = "Lying chest-down on an incline bench stops you swinging — all the work goes to your back, none to momentum or your lower back.",
            whenToUse = "You want rows without loading the lower back, or you keep heaving single-arm rows."),
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

        // ── Shoulders ──────────────────────────────────────────────────────────────
        ExerciseDef("db-lateral-raise", "DB Lateral Raise", MuscleGroup.SHOULDERS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 4, "12-15", "Priority — slow eccentric",
            muscleTarget = "Side delts (the cap on top of your shoulder that makes shoulders look WIDE)",
            why = "The single best exercise for shoulder width. Big visual lever for looking built in a tee.",
            whenToUse = "Default pick. The shoulder exercise you should never skip."),
        ExerciseDef("cable-lateral-raise", "Cable Lateral Raise", MuscleGroup.SHOULDERS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Constant tension",
            muscleTarget = "Side delts",
            why = "MWM-989 low pulley with D-handle. Cables give constant tension the whole movement — DBs are easy at the bottom and hard at the top.",
            whenToUse = "Often grows shoulders faster than DBs once you've done both a while."),
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
            whenToUse = "Front delts lag, or for variety alongside laterals."),
        ExerciseDef("lean-away-cable-lateral", "Lean-Away Cable Lateral", MuscleGroup.SHOULDERS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.INTERMEDIATE, 3, "12-15", "Lean away for a deep stretch",
            muscleTarget = "Side delts, maximum stretch",
            why = "Stand slightly leaned away from the MWM-989 low pulley — the side delt stretches fully at the bottom.",
            whenToUse = "You've done regular laterals for months and want more growth from the same muscle."),

        // ── Rear delts ─────────────────────────────────────────────────────────────
        ExerciseDef("face-pull", "Face Pull (cable)", MuscleGroup.REAR_DELTS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "15", "Posture fix — non-negotiable",
            muscleTarget = "Back of shoulders + upper back (fixes posture)",
            why = "Builds the rear shoulder AND counteracts forward shoulders from gaming and desk time. Non-negotiable for posture.",
            whenToUse = "Default pick. Do these even if you skip everything else."),
        ExerciseDef("db-rear-delt-fly", "Rear Delt DB Fly", MuscleGroup.REAR_DELTS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "12-15", "Bent over, raise like wings",
            muscleTarget = "Back of shoulders",
            why = "Bend over at the waist, raise dumbbells out to the sides like wings. Same target as face pulls but with DBs.",
            whenToUse = "Variety. Or warm-up."),
        ExerciseDef("cable-rear-delt-fly", "Bent-Over Cable Rear Fly", MuscleGroup.REAR_DELTS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.INTERMEDIATE, 3, "12-15", "Cables crossed, pull apart",
            muscleTarget = "Back of shoulders, constant tension",
            why = "MWM-989 cables crossed in front, bend over, pull apart. Cables stay loaded the whole movement.",
            whenToUse = "Want a different feel than DB rear flies."),

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
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "10-12", "Stretched bicep = growth",
            muscleTarget = "Long head of bicep (the peak)",
            why = "The stretched position at the bottom is what builds the bicep \"peak\" you see when flexing.",
            whenToUse = "You've done hammer curls for a while and want to build the bicep peak specifically."),
        ExerciseDef("preacher-curl", "Preacher Curl (MWM pad)", MuscleGroup.BICEPS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-12", "Pad prevents cheating",
            muscleTarget = "Lower bicep (forces strict form)",
            why = "Use the preacher pad on the MWM-989 attached to the low cable. The pad prevents cheating with momentum.",
            whenToUse = "You catch yourself swinging the weight on regular curls."),
        ExerciseDef("db-concentration-curl", "DB Concentration Curl", MuscleGroup.BICEPS,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "12-15", "Elbow braced on inner thigh",
            muscleTarget = "Bicep peak, one arm at a time",
            why = "Sit on the bench, elbow braced on inner thigh, curl one arm. Classic bodybuilder finisher.",
            whenToUse = "End-of-workout finisher, or to fix arm size imbalance."),
        ExerciseDef("cable-curl", "Cable Curl (low pulley)", MuscleGroup.BICEPS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-12", "Constant tension",
            muscleTarget = "Whole bicep, constant tension",
            why = "MWM-989 low pulley with bar or rope attachment. Constant tension the whole movement.",
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
        ExerciseDef("cable-tricep-pushdown", "Cable Tricep Pushdown", MuscleGroup.TRICEPS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "MWM-989 high pulley",
            muscleTarget = "Lateral head of tricep (the outer part)",
            why = "MWM-989 high pulley with bar or rope. Builds the outer tricep — most visible from the side.",
            whenToUse = "Variety. Or as a finisher after pressing."),
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
            whenToUse = "No DBs free, or as a tricep finisher."),
        ExerciseDef("diamond-push-up", "Diamond Push-Up", MuscleGroup.TRICEPS,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(COMP, BW), Difficulty.INTERMEDIATE, 3, "AMRAP", "Hands in a diamond shape",
            muscleTarget = "Triceps + inner chest",
            why = "Push-up with hands close together in a diamond shape. No equipment needed.",
            whenToUse = "Tricep finisher, or no equipment available."),

        // ── Quads ─────────────────────────────────────────────────────────────────
        ExerciseDef("goblet-squat", "Goblet Squat", MuscleGroup.QUADS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 4, "10-12", "Heaviest DB you have",
            muscleTarget = "Quads (front of thigh) + glutes",
            why = "Easiest squat variation to learn. Holding the weight in front forces a good upright posture.",
            whenToUse = "Default pick until you have heavier DBs."),
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
        ExerciseDef("db-hip-thrust", "DB Hip Thrust", MuscleGroup.GLUTES,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "10-12", "Upper back on bench, DB on hips",
            muscleTarget = "Glutes (max load, isolated)",
            why = "Back against the bench, DB on hips, drive hips up. Lets you load glutes heavier than any other exercise.",
            whenToUse = "Lower back issues — this is back-friendly. Or to grow glutes specifically."),
        ExerciseDef("db-glute-bridge", "DB Glute Bridge", MuscleGroup.GLUTES,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "12-15", "On the floor, DB on hips",
            muscleTarget = "Glutes + hamstrings",
            why = "Lie on floor, DB on hips, drive hips up. Easy on the back, hits glutes and hamstrings together.",
            whenToUse = "Low back is sore. Or as a finisher."),
        ExerciseDef("cable-glute-kickback", "Cable Glute Kickback", MuscleGroup.GLUTES,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "Ankle strap, kick straight back",
            muscleTarget = "Glutes only (isolation)",
            why = "MWM-989 low pulley with ankle strap (or loop the cable around your foot). Kick one leg straight back.",
            whenToUse = "Pure glute isolation. Or as a finisher."),

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
            whenToUse = "No equipment, or to fix imbalance between calves."),

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
            whenToUse = "No pull-up bar, or as a warm-up to other core work."),
        ExerciseDef("cable-crunch", "Cable Crunch", MuscleGroup.CORE,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-15", "Loaded abs",
            muscleTarget = "Upper abs (six-pack), loadable",
            why = "MWM-989 high pulley, kneel facing the machine, crunch down. Lets you ADD WEIGHT to ab work.",
            whenToUse = "You want six-pack development with actual weight progression."),
        ExerciseDef("lying-leg-raise", "Lying Leg Raise", MuscleGroup.CORE,
            listOf(Equipment.BODYWEIGHT_ONLY), ExerciseUnit.BODYWEIGHT,
            listOf(ISO, BW), Difficulty.BEGINNER, 3, "12-15", "Lower slowly, don't arch",
            muscleTarget = "Lower abs",
            why = "Lie on the floor, raise straight legs to vertical, lower slowly without touching the ground.",
            whenToUse = "No pull-up bar available.")
    )

    private val byId: Map<String, ExerciseDef> = all.associateBy { it.id }
    private val byName: Map<String, ExerciseDef> = all.associateBy { it.name }

    fun byId(id: String): ExerciseDef? = byId[id]
    fun byName(name: String): ExerciseDef? = byName[name]

    fun forMuscle(muscle: MuscleGroup): List<ExerciseDef> = all.filter { it.muscle == muscle }

    /**
     * Movement pattern per exercise (program-unlock Phase 4). Only the compound / core movements are
     * listed; everything else is single-joint accessory work → [MovementPattern.ISOLATION] by default.
     * Kept as one compact map rather than a field on all 52 entries — single source, easy to tweak.
     */
    private val patterns: Map<String, MovementPattern> = mapOf(
        // Pressing
        "db-bench-press" to MovementPattern.HORIZONTAL_PUSH,
        "incline-db-bench-press" to MovementPattern.HORIZONTAL_PUSH,
        "machine-chest-press" to MovementPattern.HORIZONTAL_PUSH,
        "push-up" to MovementPattern.HORIZONTAL_PUSH,
        "close-grip-db-press" to MovementPattern.HORIZONTAL_PUSH,
        "diamond-push-up" to MovementPattern.HORIZONTAL_PUSH,
        "db-overhead-press" to MovementPattern.VERTICAL_PUSH,
        "bench-dip" to MovementPattern.VERTICAL_PUSH,
        // Pulling
        "lat-pulldown" to MovementPattern.VERTICAL_PULL,
        "close-grip-lat-pulldown" to MovementPattern.VERTICAL_PULL,
        "pull-up" to MovementPattern.VERTICAL_PULL,
        "chin-up" to MovementPattern.VERTICAL_PULL,
        "machine-seated-row" to MovementPattern.HORIZONTAL_PULL,
        "db-row" to MovementPattern.HORIZONTAL_PULL,
        "chest-supported-db-row" to MovementPattern.HORIZONTAL_PULL,
        // Legs
        "goblet-squat" to MovementPattern.SQUAT,
        "db-bulgarian-split-squat" to MovementPattern.LUNGE,
        "db-reverse-lunge" to MovementPattern.LUNGE,
        "db-step-up" to MovementPattern.LUNGE,
        "db-walking-lunge" to MovementPattern.LUNGE,
        "db-romanian-deadlift" to MovementPattern.HINGE,
        "db-stiff-leg-deadlift" to MovementPattern.HINGE,
        "db-single-leg-rdl" to MovementPattern.HINGE,
        "db-hip-thrust" to MovementPattern.HINGE,
        "db-glute-bridge" to MovementPattern.HINGE,
        // Core
        "hanging-knee-raise" to MovementPattern.CORE,
        "plank" to MovementPattern.CORE,
        "cable-crunch" to MovementPattern.CORE,
        "lying-leg-raise" to MovementPattern.CORE
    )

    fun patternOf(def: ExerciseDef): MovementPattern =
        patterns[def.id] ?: MovementPattern.ISOLATION

    /**
     * Which problem areas each movement stresses (program-unlock Phase 3). Only listed exercises have
     * contraindications; everything else is unflagged. A movement can stress more than one area.
     */
    private val contraindications: Map<String, Set<ProblemArea>> = mapOf(
        // Knee-loading
        "goblet-squat" to setOf(ProblemArea.KNEES),
        "db-bulgarian-split-squat" to setOf(ProblemArea.KNEES),
        "db-reverse-lunge" to setOf(ProblemArea.KNEES),
        "db-step-up" to setOf(ProblemArea.KNEES),
        "db-walking-lunge" to setOf(ProblemArea.KNEES),
        "leg-extension" to setOf(ProblemArea.KNEES),
        // Shoulder-stressing (overhead / dips / front delt)
        "db-overhead-press" to setOf(ProblemArea.SHOULDERS),
        "db-front-raise" to setOf(ProblemArea.SHOULDERS),
        "bench-dip" to setOf(ProblemArea.SHOULDERS, ProblemArea.WRISTS),
        // Spinal-loading hip hinges / bent-over
        "db-romanian-deadlift" to setOf(ProblemArea.LOWER_BACK),
        "db-stiff-leg-deadlift" to setOf(ProblemArea.LOWER_BACK),
        "db-single-leg-rdl" to setOf(ProblemArea.LOWER_BACK),
        "db-row" to setOf(ProblemArea.LOWER_BACK),
        // Wrist-loading (bodyweight on hands)
        "push-up" to setOf(ProblemArea.WRISTS),
        "diamond-push-up" to setOf(ProblemArea.WRISTS),
        "plank" to setOf(ProblemArea.WRISTS)
    )

    fun contraindicationsOf(def: ExerciseDef): Set<ProblemArea> =
        contraindications[def.id] ?: emptySet()

    /** Available if no equipment is configured (empty = all) or every required item is on hand. */
    fun isAvailable(def: ExerciseDef, available: Set<Equipment>): Boolean =
        available.isEmpty() || def.equipment.all { it == Equipment.BODYWEIGHT_ONLY || it in available }

    /**
     * Swap candidates for [muscle], drawn from the single pool the generator uses: excludes
     * [disliked] exercises and anything that needs equipment not in [available] (empty = all).
     */
    fun swapCandidates(
        muscle: MuscleGroup,
        available: Set<Equipment>,
        disliked: Set<String>
    ): List<ExerciseDef> = forMuscle(muscle).filter { it.id !in disliked && isAvailable(it, available) }
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
