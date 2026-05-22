package com.forge.app.program

/**
 * Per-exercise form instructions, keyed by [ExercisePlan.id]. Lives in its own file
 * because the tutorial strings are long and would balloon [Program] beyond comfortable
 * reading length. Lookup is via [forExercise] — exercises without a tutorial return null
 * (e.g. swaps that didn't make it into the daily plan; their tutorials can be added
 * here if needed).
 *
 * lb4 (Goblet Squat on Lower B) deliberately reuses la1's instructions — the only
 * difference is rep target, called out at the end.
 */
object Tutorials {

    val byExerciseId: Map<String, String> = mapOf(
        "ua1" to "Lie on the bench, dumbbells at chest level with palms facing forward. Press straight up until arms are almost locked, then slowly lower back to chest (3 seconds down). Keep your feet flat on the floor and squeeze your shoulder blades together against the bench. Common mistake: bouncing the weight off your chest.",

        "ua2" to "Sit on the machine, grab the handles at chest level. Push the handles forward smoothly until arms are almost straight, then slowly return. Keep your back against the pad — don't arch off it. Common mistake: locking elbows hard at the top, which takes load off the chest.",

        "ua3" to "Grab the bar wider than shoulder-width, palms facing forward. Sit down, lean back slightly. Pull the bar down to your upper chest by driving your elbows down and back — think \"pull with the elbows, not the hands.\" Squeeze your back at the bottom. Common mistake: pulling behind the neck (bad for shoulders) or using too much weight and swinging.",

        "ua4" to "Stand with a dumbbell in each hand, arms by your sides. Raise them out to the sides (like making a T) until elbows are at shoulder height. Slight bend in elbows the whole time. Lower SLOWLY (3 seconds). Imagine pouring water out of the dumbbell at the top. Common mistake: going too heavy and shrugging — keep traps relaxed.",

        "ua5" to "Hold one dumbbell with both hands behind your head, elbows pointing up. Extend your arms straight up, then lower the dumbbell slowly behind your head. Keep your elbows close to your ears — they shouldn't flare out. This stretches the long head of the tricep, which is what gives the back of your arm visible size.",

        "ua6" to "Stand with dumbbells at your sides, palms facing each other (like holding hammers). Curl up without rotating your wrists. Squeeze at the top, lower slowly. Common mistake: swinging the weight with your back — keep elbows pinned to your sides.",

        "la1" to "Hold one dumbbell vertically against your chest, both hands cupping the top end. Feet shoulder-width apart, toes slightly out. Squat down by pushing your knees out and hips back, going as deep as you can with a flat back. Drive through your heels to stand. Common mistake: knees caving in or heels lifting.",

        "la2" to "Hold dumbbells in front of your thighs, knees soft (slight bend, don't lock or squat). Push your hips BACK while lowering the dumbbells down the front of your legs. Stop when you feel a strong stretch in your hamstrings (usually around mid-shin). Drive hips forward to stand. Keep your back flat the whole time — never round. This is the single best exercise for posture if done right.",

        "la3" to "Sit on the machine, pad on your shins just above the ankles. Extend your legs straight out by squeezing your quads. Pause at the top for 1 second, lower slowly. Don't use momentum or kick the weight up.",

        "la4" to "Dumbbells at your sides. Step forward with one leg, lowering until both knees are at 90°. Front shin should be vertical, back knee almost touches the floor. Push off the front heel to step forward into the next lunge. Keep your torso upright.",

        "la5" to "Hold a dumbbell in one hand, stand on the edge of a step (or flat floor). Rise up onto your toes as high as you can, squeeze at the top for 1 second, lower slowly until heels are below the step. Hold something for balance with your free hand.",

        "la6" to "Hang from a pull-up bar, arms straight. Bring your knees up toward your chest by curling your pelvis up (not just lifting your legs). Lower slowly with control. Don't swing. If hanging is too hard, lying leg raises on the floor work too.",

        "ub1" to "Sit on the machine, feet on the platform, slight bend in knees. Grab handles with arms extended. Pull the handles to your stomach by driving your elbows back and squeezing your shoulder blades together. Pause for 1 second when handles touch (or get close to) your stomach. Slowly return. Don't use your lower back to swing.",

        "ub2" to "Set the bench to about 30° incline. Same form as flat DB bench: press up, lower slow. The incline targets the upper part of the chest, which is what fills out the top of a t-shirt and gives chest shape. Common mistake: bench angle too steep (over 45°) — that turns it into mostly a shoulder exercise.",

        "ub3" to "Same as regular lat pulldown but with a narrow grip (hands close together, palms facing you or facing each other if you have a V-handle attachment). This hits the lower lats and feels more like a \"pulling with the biceps\" movement. Pull to the upper chest, squeeze at the bottom.",

        "ub4" to "Same as Upper A. Yes, we do it again. Side delts respond well to high frequency and your shoulders are a priority for the V-taper look.",

        "ub5" to "Lie on the bench with a dumbbell in each hand, arms straight up. Bend ONLY at the elbows to lower the dumbbells toward your forehead (don't actually hit your skull — that's the joke in the name). Keep your upper arms locked vertical. Extend back to start. The fixed upper arm is what isolates the triceps.",

        "ub6" to "Set the bench to 45-60° incline. Sit back with dumbbells hanging at your sides, palms forward. Let your arms hang fully back (this stretches the bicep). Curl up without moving your elbows forward, squeeze at the top, lower slowly to the full stretch. The stretched position is what makes this version better than standing curls for growth.",

        "ub7" to "Set the MWM-989 cable at face height (or as high as you can). Use a rope or bar attachment. Pull the handle toward your face, elbows high, ending with hands beside your ears. Imagine pulling the rope APART at the end. This trains the rear shoulder and upper back — fixes the rounded-shoulders look from gaming and desk time. Critical for posture.",

        "lb1" to "Stand a few feet in front of the bench, place the top of one foot back on the bench. Hold dumbbells at your sides. Lower straight down (front knee tracks over toes, back knee toward floor) until front thigh is roughly parallel to ground. Drive through the front heel to stand. ADVANCED note: balance is hard at first — try without weight for a few sessions. Massive size builder despite light weight.",

        "lb2" to "Like the Romanian deadlift, but with very little knee bend (legs nearly straight). This puts maximum stretch on the hamstrings. Same rules: hips push back, flat back, dumbbells slide down the front of your legs, stand by driving hips forward. Go only as deep as you can with a flat back.",

        "lb3" to "Lie face down (or sit, depending on machine) with the pad against the back of your ankles. Curl your heels toward your butt by squeezing the hamstrings. Pause for 1 second, lower slowly. Pure hamstring isolation — easy on the lower back.",

        "lb4" to "Same as Lower A — see that tutorial. Today is higher reps with slightly lighter weight, focusing on time under tension.",

        "lb5" to "Sit on the bench, dumbbells resting on your knees (one on each thigh, held in place with your hands). Rise up on your toes as high as you can, squeeze, lower slowly. The bent knee position targets the soleus — a different calf muscle than standing raises hit.",

        "lb6" to "Kneel in front of the MWM-989 high pulley with a rope or bar. Hold the handle by your head/ears. Crunch DOWN by curling your torso (think rolling your ribs to your hips). Don't pull with your arms — they're just holding the weight in place. Squeeze abs hard at the bottom."
    )

    fun forExercise(id: String): String? = byExerciseId[id]
}
