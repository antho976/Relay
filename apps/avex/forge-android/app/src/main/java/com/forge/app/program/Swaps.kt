package com.forge.app.program

/**
 * A swap candidate — an alternative exercise the user can pick for a given muscle.
 * [muscleTarget] is the human-readable description of what the swap hits (e.g.
 * "Lower lats + biceps assist"). [why] and [whenToUse] are the rationale and
 * situational guidance shown on the swap picker.
 */
data class Swap(
    val name: String,
    val unit: ExerciseUnit,
    val difficulty: Difficulty,
    val muscleTarget: String,
    val why: String,
    val whenToUse: String
)

/**
 * The full swap catalog, grouped by muscle. The order within each list is the
 * intended sort: default pick first, then variants from most common to most niche.
 */
object Swaps {

    val byMuscle: Map<MuscleGroup, List<Swap>> = mapOf(
        MuscleGroup.CHEST to listOf(
            Swap(
                name = "DB Bench Press",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Whole chest (middle and lower)",
                why = "The classic chest builder. Hits the whole muscle and lets you load it heavy as you grow.",
                whenToUse = "Default pick. If you can press dumbbells without shoulder pain, use this."
            ),
            Swap(
                name = "Incline DB Bench Press",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Upper chest (the part near your collarbone)",
                why = "Builds the top of your chest, which is what fills out a t-shirt at the neckline.",
                whenToUse = "When your lower chest is catching up but the top still looks flat. Or if regular bench bores you."
            ),
            Swap(
                name = "Machine Chest Press",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Whole chest, fixed path",
                why = "Machine guides the movement, so you can't mess up the form. Easier on shoulders than free weights.",
                whenToUse = "Shoulder feels tweaky. Or your last session was so hard your stabilizer muscles are toast."
            ),
            Swap(
                name = "Pec Fly (cable)",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Inner chest line",
                why = "Isolates the chest with no shoulder/tricep help. Cross both MWM cables in front of you. Creates the line down the middle of your chest.",
                whenToUse = "You've already pressed and want to finish the chest off without more pressing fatigue."
            ),
            Swap(
                name = "Push-Up (Feet Elevated)",
                unit = ExerciseUnit.BODYWEIGHT,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Whole chest + shoulders + triceps",
                why = "No equipment needed. Feet on a bench makes it harder than regular push-ups.",
                whenToUse = "Equipment is in use, or as a warm-up before pressing."
            )
        ),
        MuscleGroup.BACK to listOf(
            Swap(
                name = "Lat Pulldown",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Lats (the wing muscles on the sides of your back)",
                why = "Builds back WIDTH — half of the V-taper you want. Easier to learn than pull-ups.",
                whenToUse = "Default pick. The foundation back exercise."
            ),
            Swap(
                name = "Close-Grip Lat Pulldown",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Lower lats + biceps assist",
                why = "Hits the bottom part of the lats more, easier on your shoulders than wide-grip.",
                whenToUse = "Shoulders feel tight on regular pulldowns. Or you want a different feel from wide-grip."
            ),
            Swap(
                name = "Machine Seated Row",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Mid-back (between your shoulder blades)",
                why = "Builds back THICKNESS — depth from the side. Different look than pulldowns build.",
                whenToUse = "You want a \"fuller\" back look from the side, or to balance out lots of pulldowns."
            ),
            Swap(
                name = "DB Row (single arm)",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "One side of your back at a time",
                why = "Fixes left/right imbalance. Place a knee on the bench, other foot on floor, row the DB to your hip.",
                whenToUse = "You notice one side does more work than the other. Or as a finisher."
            ),
            Swap(
                name = "Pull-Up / Chin-Up",
                unit = ExerciseUnit.BODYWEIGHT,
                difficulty = Difficulty.ADVANCED,
                muscleTarget = "Whole back + biceps",
                why = "King of bodyweight back exercises if you have a pull-up bar. Builds insane back density.",
                whenToUse = "You have a pull-up bar and can do at least 3-5 clean reps."
            )
        ),
        MuscleGroup.SHOULDERS to listOf(
            Swap(
                name = "DB Lateral Raise",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Side delts (the cap on top of your shoulder that makes shoulders look WIDE)",
                why = "The single best exercise for shoulder width. Big visual lever for looking built in a tee.",
                whenToUse = "Default pick. The shoulder exercise you should never skip."
            ),
            Swap(
                name = "Cable Lateral Raise",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Side delts",
                why = "MWM-989 low pulley with D-handle. Cables give constant tension the whole movement — DBs are easy at the bottom and hard at the top.",
                whenToUse = "Often grows shoulders faster than DBs once you've done both a while."
            ),
            Swap(
                name = "DB Overhead Press",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.INTERMEDIATE,
                muscleTarget = "Front delts + side delts",
                why = "Builds strength and overall shoulder size. Compound movement, lots of muscle worked.",
                whenToUse = "You want to focus on shoulder STRENGTH, not just size. Or to add variety."
            ),
            Swap(
                name = "Lean-Away Cable Lateral",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.INTERMEDIATE,
                muscleTarget = "Side delts, maximum stretch",
                why = "Stand slightly leaned away from the MWM-989 low pulley — the side delt stretches fully at the bottom.",
                whenToUse = "You've done regular laterals for months and want more growth from the same muscle."
            )
        ),
        MuscleGroup.REAR_DELTS to listOf(
            Swap(
                name = "Face Pull (cable)",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Back of shoulders + upper back (fixes posture)",
                why = "Builds the rear shoulder AND counteracts forward shoulders from gaming and desk time. Non-negotiable for posture.",
                whenToUse = "Default pick. Do these even if you skip everything else."
            ),
            Swap(
                name = "Rear Delt DB Fly",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Back of shoulders",
                why = "Bend over at the waist, raise dumbbells out to the sides like wings. Same target as face pulls but with DBs.",
                whenToUse = "Variety. Or warm-up."
            ),
            Swap(
                name = "Bent-Over Cable Rear Fly",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.INTERMEDIATE,
                muscleTarget = "Back of shoulders, constant tension",
                why = "MWM-989 cables crossed in front, bend over, pull apart. Cables stay loaded the whole movement.",
                whenToUse = "Want a different feel than DB rear flies."
            )
        ),
        MuscleGroup.BICEPS to listOf(
            Swap(
                name = "DB Hammer Curl",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Bicep + forearm (palms facing each other)",
                why = "Builds the bicep AND forearm. Forearms make your arm look thicker in short sleeves.",
                whenToUse = "Default pick. Works two muscles for the price of one."
            ),
            Swap(
                name = "DB Incline Curl",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Long head of bicep (the peak)",
                why = "The stretched position at the bottom is what builds the bicep \"peak\" you see when flexing.",
                whenToUse = "You've done hammer curls for a while and want to build the bicep peak specifically."
            ),
            Swap(
                name = "Preacher Curl (MWM pad)",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Lower bicep (forces strict form)",
                why = "Use the preacher pad on the MWM-989 attached to the low cable. The pad prevents cheating with momentum.",
                whenToUse = "You catch yourself swinging the weight on regular curls."
            ),
            Swap(
                name = "DB Concentration Curl",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Bicep peak, one arm at a time",
                why = "Sit on the bench, elbow braced on inner thigh, curl one arm. Classic bodybuilder finisher.",
                whenToUse = "End-of-workout finisher, or to fix arm size imbalance."
            ),
            Swap(
                name = "Cable Curl (low pulley)",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Whole bicep, constant tension",
                why = "MWM-989 low pulley with bar or rope attachment. Constant tension the whole movement.",
                whenToUse = "Variety. Often grows arms faster after months of just DB curls."
            )
        ),
        MuscleGroup.TRICEPS to listOf(
            Swap(
                name = "DB Overhead Tricep Ext.",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Long head of tricep (the back of your arm)",
                why = "The long head is the biggest part of the tricep and gives the back of your arm visible size.",
                whenToUse = "Default pick. Biggest bang-for-buck tricep exercise."
            ),
            Swap(
                name = "DB Skull Crusher",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.INTERMEDIATE,
                muscleTarget = "Whole tricep, mass builder",
                why = "Hits all three tricep heads. Builds raw tricep size.",
                whenToUse = "You want to load triceps heavier than overhead extensions allow."
            ),
            Swap(
                name = "Cable Tricep Pushdown",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Lateral head of tricep (the outer part)",
                why = "MWM-989 high pulley with bar or rope. Builds the outer tricep — most visible from the side.",
                whenToUse = "Variety. Or as a finisher after pressing."
            ),
            Swap(
                name = "Close-Grip DB Press",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Triceps + chest",
                why = "DB Bench Press with hands closer together. Compound — hits chest and triceps together.",
                whenToUse = "You want strength + size in one move. Good for short workouts."
            ),
            Swap(
                name = "Diamond Push-Up",
                unit = ExerciseUnit.BODYWEIGHT,
                difficulty = Difficulty.INTERMEDIATE,
                muscleTarget = "Triceps + inner chest",
                why = "Push-up with hands close together in a diamond shape. No equipment needed.",
                whenToUse = "Tricep finisher, or no equipment available."
            )
        ),
        MuscleGroup.QUADS to listOf(
            Swap(
                name = "Goblet Squat",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Quads (front of thigh) + glutes",
                why = "Easiest squat variation to learn. Holding the weight in front forces a good upright posture.",
                whenToUse = "Default pick until you have heavier DBs."
            ),
            Swap(
                name = "DB Bulgarian Split Squat",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.ADVANCED,
                muscleTarget = "Quads + glutes, one leg at a time",
                why = "Builds huge legs with light weight because all the load is on one leg. Fixes imbalances.",
                whenToUse = "Light DBs only — works great. Hard to balance at first, give it 2-3 sessions."
            ),
            Swap(
                name = "DB Walking Lunge",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Quads + glutes, moving",
                why = "Easier balance than Bulgarian split squats. Glutes get more work because you step forward.",
                whenToUse = "Want unilateral leg work without the balance challenge of Bulgarians."
            ),
            Swap(
                name = "Leg Extension",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Quads only (pure isolation)",
                why = "MWM-989 leg developer. Hits quads without any glute, back, or balance involvement.",
                whenToUse = "After squats to fully fatigue the quads. Or when you don't want to load your spine."
            ),
            Swap(
                name = "DB Step-Up (on bench)",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Quads + glutes, unilateral",
                why = "Step up onto your bench with DBs. Simple, scalable, great for beginners.",
                whenToUse = "Beginner-friendly unilateral work. Or as a warm-up."
            )
        ),
        MuscleGroup.HAMSTRINGS to listOf(
            Swap(
                name = "DB Romanian Deadlift",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.INTERMEDIATE,
                muscleTarget = "Hamstrings + glutes + spinal erectors (low back)",
                why = "Best all-around posterior chain builder. Also fixes posture and builds the \"athletic back\" look.",
                whenToUse = "Default pick. Do these."
            ),
            Swap(
                name = "DB Stiff-Leg Deadlift",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.INTERMEDIATE,
                muscleTarget = "Hamstrings (max stretch)",
                why = "Almost-straight legs put more stretch on the hamstrings, which is what builds size.",
                whenToUse = "Variation on RDLs. More hamstring, less glute and back."
            ),
            Swap(
                name = "Leg Curl",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Hamstrings only",
                why = "MWM-989 leg developer. Pure hamstring isolation — no spinal load, no balance.",
                whenToUse = "Low back is tired or sore. Or to finish hamstrings after RDLs."
            ),
            Swap(
                name = "Single-Leg RDL",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.ADVANCED,
                muscleTarget = "Hamstrings + glutes, one leg + balance",
                why = "Hold one DB, hinge on one leg while the other extends straight back. Builds insane balance.",
                whenToUse = "You've mastered regular RDLs and want a new challenge."
            ),
            Swap(
                name = "Glute Bridge (DB on hips)",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Hamstrings + glutes",
                why = "Lie on floor, DB on hips, drive hips up. Easy on the back, hits glutes and hamstrings together.",
                whenToUse = "Low back is sore. Or as a finisher."
            )
        ),
        MuscleGroup.GLUTES to listOf(
            Swap(
                name = "DB Walking Lunge",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Glutes + quads, moving",
                why = "The walking pattern recruits glutes more than stationary leg exercises.",
                whenToUse = "Default pick. Easy to learn, hits glutes well."
            ),
            Swap(
                name = "DB Hip Thrust (on bench)",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Glutes (max load, isolated)",
                why = "Back against the bench, DB on hips, drive hips up. Lets you load glutes heavier than any other exercise.",
                whenToUse = "Lower back issues — this is back-friendly. Or to grow glutes specifically."
            ),
            Swap(
                name = "DB Bulgarian Split Squat",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.ADVANCED,
                muscleTarget = "Glutes + quads, one leg",
                why = "Going deep on Bulgarian split squats absolutely smokes the glutes.",
                whenToUse = "Want both quad AND glute size from one exercise."
            ),
            Swap(
                name = "DB Step-Up",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Glutes + quads",
                why = "Step onto the bench with DBs. Higher bench = more glute work.",
                whenToUse = "Beginner-friendly glute work."
            ),
            Swap(
                name = "Cable Glute Kickback",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Glutes only (isolation)",
                why = "MWM-989 low pulley with ankle strap (or loop the cable around your foot). Kick one leg straight back.",
                whenToUse = "Pure glute isolation. Or as a finisher."
            )
        ),
        MuscleGroup.CALVES to listOf(
            Swap(
                name = "Standing Calf Raise",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Upper calf (gastrocnemius — the diamond shape)",
                why = "Standing position with straight legs targets the bigger, more visible part of the calf.",
                whenToUse = "Default pick."
            ),
            Swap(
                name = "Seated Calf Raise",
                unit = ExerciseUnit.DUMBBELL,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Lower calf (soleus — the flat muscle underneath)",
                why = "Bent-knee position targets a DIFFERENT calf muscle than standing raises.",
                whenToUse = "Train both standing and seated for full calf development."
            ),
            Swap(
                name = "Single-Leg Calf Raise",
                unit = ExerciseUnit.BODYWEIGHT,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "One calf at a time",
                why = "No equipment needed. Bodyweight is plenty for calves.",
                whenToUse = "No equipment, or to fix imbalance between calves."
            )
        ),
        MuscleGroup.CORE to listOf(
            Swap(
                name = "Hanging Knee Raise",
                unit = ExerciseUnit.BODYWEIGHT,
                difficulty = Difficulty.INTERMEDIATE,
                muscleTarget = "Lower abs + grip strength",
                why = "Forces lower abs to do the work. Most ab exercises miss the lower part.",
                whenToUse = "Default pick if you have a pull-up bar."
            ),
            Swap(
                name = "Plank (timed)",
                unit = ExerciseUnit.BODYWEIGHT,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Whole core, isometric",
                why = "Builds the bracing strength that protects your back during squats and deadlifts.",
                whenToUse = "No pull-up bar, or as a warm-up to other core work."
            ),
            Swap(
                name = "Cable Crunch",
                unit = ExerciseUnit.PLATES,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Upper abs (six-pack), loadable",
                why = "MWM-989 high pulley, kneel facing the machine, crunch down. Lets you ADD WEIGHT to ab work.",
                whenToUse = "You want six-pack development with actual weight progression."
            ),
            Swap(
                name = "Lying Leg Raise",
                unit = ExerciseUnit.BODYWEIGHT,
                difficulty = Difficulty.BEGINNER,
                muscleTarget = "Lower abs",
                why = "Lie on the floor, raise straight legs to vertical, lower slowly without touching ground.",
                whenToUse = "No pull-up bar available."
            )
        )
    )

    fun forMuscle(muscle: MuscleGroup): List<Swap> = byMuscle[muscle].orEmpty()
}
