import React, { useState, useEffect, useRef } from 'react';
import {
  ChevronLeft, Plus, Minus, Check, X, FileDown, RotateCcw, Play, Pause,
  Repeat, StickyNote, ChevronDown, ChevronUp, Info, ExternalLink, Edit2,
  BarChart3, Home, AlertCircle, Flame, Smile, Frown, Meh, Zap,
  SkipForward, Trophy, Minimize2, HelpCircle, Activity, Calendar,
} from 'lucide-react';

const PLATE_LB = 15;
const STORAGE_KEY = 'forge-data-v5';
const DELOAD_AFTER_SESSIONS = 24;
const REST_DEFAULT = 150;

const DEFAULT_DAY_NAMES = {
  'upper-a': 'Chest, Shoulders & Arms',
  'lower-a': 'Legs — Quad focus',
  'upper-b': 'Back, Arms & Posture',
  'lower-b': 'Legs — Hamstring focus',
};

const PROGRAM = {
  'upper-a': {
    subtitle: 'Push-leaning · Size focus',
    word: 'PUSH',
    color: '#e85d4a',
    warmup: [
      'Arm circles — 10 forward, 10 back',
      'Push-ups — 10 slow reps',
      'Light shoulder press with empty hands — 15 reps',
      'Scapular wall slides — 10 reps',
    ],
    exercises: [
      { id: 'ua1', name: 'DB Bench Press', sets: 3, reps: '8-10', unit: 'db', muscle: 'chest', difficulty: 'beginner', note: '1-2 reps shy of failure',
        tutorial: 'Lie on the bench, dumbbells at chest level with palms facing forward. Press straight up until arms are almost locked, then slowly lower back to chest (3 seconds down). Keep your feet flat on the floor and squeeze your shoulder blades together against the bench. Common mistake: bouncing the weight off your chest.' },
      { id: 'ua2', name: 'Machine Chest Press', sets: 3, reps: '10-12', unit: 'plates', muscle: 'chest', difficulty: 'beginner', note: 'MWM-989 press arm',
        tutorial: 'Sit on the machine, grab the handles at chest level. Push the handles forward smoothly until arms are almost straight, then slowly return. Keep your back against the pad — don\'t arch off it. Common mistake: locking elbows hard at the top, which takes load off the chest.' },
      { id: 'ua3', name: 'Lat Pulldown', sets: 4, reps: '8-12', unit: 'plates', muscle: 'back', difficulty: 'beginner', note: 'Wide grip, pull to upper chest',
        tutorial: 'Grab the bar wider than shoulder-width, palms facing forward. Sit down, lean back slightly. Pull the bar down to your upper chest by driving your elbows down and back — think "pull with the elbows, not the hands." Squeeze your back at the bottom. Common mistake: pulling behind the neck (bad for shoulders) or using too much weight and swinging.' },
      { id: 'ua4', name: 'DB Lateral Raise', sets: 4, reps: '12-15', unit: 'db', muscle: 'shoulders', difficulty: 'beginner', note: 'Priority — slow eccentric',
        tutorial: 'Stand with a dumbbell in each hand, arms by your sides. Raise them out to the sides (like making a T) until elbows are at shoulder height. Slight bend in elbows the whole time. Lower SLOWLY (3 seconds). Imagine pouring water out of the dumbbell at the top. Common mistake: going too heavy and shrugging — keep traps relaxed.' },
      { id: 'ua5', name: 'DB Overhead Tricep Ext.', sets: 3, reps: '10-12', unit: 'db', muscle: 'triceps', difficulty: 'beginner', note: 'Long head — biggest visual lever',
        tutorial: 'Hold one dumbbell with both hands behind your head, elbows pointing up. Extend your arms straight up, then lower the dumbbell slowly behind your head. Keep your elbows close to your ears — they shouldn\'t flare out. This stretches the long head of the tricep, which is what gives the back of your arm visible size.' },
      { id: 'ua6', name: 'DB Hammer Curl', sets: 3, reps: '10-12', unit: 'db', muscle: 'biceps', difficulty: 'beginner', note: 'Bicep + forearm',
        tutorial: 'Stand with dumbbells at your sides, palms facing each other (like holding hammers). Curl up without rotating your wrists. Squeeze at the top, lower slowly. Common mistake: swinging the weight with your back — keep elbows pinned to your sides.' },
    ],
  },
  'lower-a': {
    subtitle: 'Quad-leaning',
    word: 'QUADS',
    color: '#d4a017',
    warmup: [
      'Bodyweight squats — 15 reps slow',
      'Leg swings — 10 each leg, forward and side',
      'Walking lunges — 10 steps',
      'Hip circles — 10 each direction',
    ],
    exercises: [
      { id: 'la1', name: 'Goblet Squat', sets: 4, reps: '10-12', unit: 'db', muscle: 'quads', difficulty: 'beginner', note: 'Heaviest DB you have',
        tutorial: 'Hold one dumbbell vertically against your chest, both hands cupping the top end. Feet shoulder-width apart, toes slightly out. Squat down by pushing your knees out and hips back, going as deep as you can with a flat back. Drive through your heels to stand. Common mistake: knees caving in or heels lifting.' },
      { id: 'la2', name: 'DB Romanian Deadlift', sets: 4, reps: '8-10', unit: 'db', muscle: 'hamstrings', difficulty: 'intermediate', note: 'Posture work too',
        tutorial: 'Hold dumbbells in front of your thighs, knees soft (slight bend, don\'t lock or squat). Push your hips BACK while lowering the dumbbells down the front of your legs. Stop when you feel a strong stretch in your hamstrings (usually around mid-shin). Drive hips forward to stand. Keep your back flat the whole time — never round. This is the single best exercise for posture if done right.' },
      { id: 'la3', name: 'Leg Extension', sets: 3, reps: '12-15', unit: 'plates', muscle: 'quads', difficulty: 'beginner', note: 'MWM-989 leg developer',
        tutorial: 'Sit on the machine, pad on your shins just above the ankles. Extend your legs straight out by squeezing your quads. Pause at the top for 1 second, lower slowly. Don\'t use momentum or kick the weight up.' },
      { id: 'la4', name: 'DB Walking Lunge', sets: 3, reps: '10/leg', unit: 'db', muscle: 'glutes', difficulty: 'beginner', note: 'Unilateral balance',
        tutorial: 'Dumbbells at your sides. Step forward with one leg, lowering until both knees are at 90°. Front shin should be vertical, back knee almost touches the floor. Push off the front heel to step forward into the next lunge. Keep your torso upright.' },
      { id: 'la5', name: 'Standing Calf Raise', sets: 4, reps: '12-15', unit: 'db', muscle: 'calves', difficulty: 'beginner', note: 'DB in hand',
        tutorial: 'Hold a dumbbell in one hand, stand on the edge of a step (or flat floor). Rise up onto your toes as high as you can, squeeze at the top for 1 second, lower slowly until heels are below the step. Hold something for balance with your free hand.' },
      { id: 'la6', name: 'Hanging Knee Raise', sets: 3, reps: '10-15', unit: 'bw', muscle: 'core', difficulty: 'intermediate', note: 'Or plank 30-60s',
        tutorial: 'Hang from a pull-up bar, arms straight. Bring your knees up toward your chest by curling your pelvis up (not just lifting your legs). Lower slowly with control. Don\'t swing. If hanging is too hard, lying leg raises on the floor work too.' },
    ],
  },
  'upper-b': {
    subtitle: 'Pull-leaning · Arm emphasis',
    word: 'PULL',
    color: '#5b9279',
    warmup: [
      'Dead hangs from bar — 20 seconds',
      'Scapular pull-ups — 10 reps',
      'Cat-cow stretches — 10 reps',
      'Light face pulls on the machine — 15 reps',
    ],
    exercises: [
      { id: 'ub1', name: 'Machine Seated Row', sets: 4, reps: '8-12', unit: 'plates', muscle: 'back', difficulty: 'beginner', note: 'Mid-back thickness',
        tutorial: 'Sit on the machine, feet on the platform, slight bend in knees. Grab handles with arms extended. Pull the handles to your stomach by driving your elbows back and squeezing your shoulder blades together. Pause for 1 second when handles touch (or get close to) your stomach. Slowly return. Don\'t use your lower back to swing.' },
      { id: 'ub2', name: 'Incline DB Bench Press', sets: 3, reps: '8-10', unit: 'db', muscle: 'chest', difficulty: 'beginner', note: 'Fills tee neckline',
        tutorial: 'Set the bench to about 30° incline. Same form as flat DB bench: press up, lower slow. The incline targets the upper part of the chest, which is what fills out the top of a t-shirt and gives chest shape. Common mistake: bench angle too steep (over 45°) — that turns it into mostly a shoulder exercise.' },
      { id: 'ub3', name: 'Close-Grip Lat Pulldown', sets: 3, reps: '10-12', unit: 'plates', muscle: 'back', difficulty: 'beginner', note: 'Different angle',
        tutorial: 'Same as regular lat pulldown but with a narrow grip (hands close together, palms facing you or facing each other if you have a V-handle attachment). This hits the lower lats and feels more like a "pulling with the biceps" movement. Pull to the upper chest, squeeze at the bottom.' },
      { id: 'ub4', name: 'DB Lateral Raise', sets: 4, reps: '12-15', unit: 'db', muscle: 'shoulders', difficulty: 'beginner', note: 'Twice a week, by design',
        tutorial: 'Same as Upper A. Yes, we do it again. Side delts respond well to high frequency and your shoulders are a priority for the V-taper look.' },
      { id: 'ub5', name: 'DB Skull Crusher', sets: 3, reps: '10-12', unit: 'db', muscle: 'triceps', difficulty: 'intermediate', note: 'Tricep mass',
        tutorial: 'Lie on the bench with a dumbbell in each hand, arms straight up. Bend ONLY at the elbows to lower the dumbbells toward your forehead (don\'t actually hit your skull — that\'s the joke in the name). Keep your upper arms locked vertical. Extend back to start. The fixed upper arm is what isolates the triceps.' },
      { id: 'ub6', name: 'DB Incline Curl', sets: 3, reps: '10-12', unit: 'db', muscle: 'biceps', difficulty: 'beginner', note: 'Stretched bicep = growth',
        tutorial: 'Set the bench to 45-60° incline. Sit back with dumbbells hanging at your sides, palms forward. Let your arms hang fully back (this stretches the bicep). Curl up without moving your elbows forward, squeeze at the top, lower slowly to the full stretch. The stretched position is what makes this version better than standing curls for growth.' },
      { id: 'ub7', name: 'Face Pull (cable)', sets: 3, reps: '15', unit: 'plates', muscle: 'rear-delts', difficulty: 'beginner', note: 'Posture fix — non-negotiable',
        tutorial: 'Set the MWM-989 cable at face height (or as high as you can). Use a rope or bar attachment. Pull the handle toward your face, elbows high, ending with hands beside your ears. Imagine pulling the rope APART at the end. This trains the rear shoulder and upper back — fixes the rounded-shoulders look from gaming and desk time. Critical for posture.' },
    ],
  },
  'lower-b': {
    subtitle: 'Hamstring & glute-leaning',
    word: 'HAMS',
    color: '#7b6cb5',
    warmup: [
      'Bodyweight squats — 15 reps',
      'Glute bridges — 15 reps',
      'Leg swings — 10 each leg',
      'Walking knee hugs — 10 each leg',
    ],
    exercises: [
      { id: 'lb1', name: 'DB Bulgarian Split Squat', sets: 4, reps: '8-10/leg', unit: 'db', muscle: 'quads', difficulty: 'advanced', note: 'Brutal but it works',
        tutorial: 'Stand a few feet in front of the bench, place the top of one foot back on the bench. Hold dumbbells at your sides. Lower straight down (front knee tracks over toes, back knee toward floor) until front thigh is roughly parallel to ground. Drive through the front heel to stand. ADVANCED note: balance is hard at first — try without weight for a few sessions. Massive size builder despite light weight.' },
      { id: 'lb2', name: 'DB Stiff-Leg Deadlift', sets: 3, reps: '10-12', unit: 'db', muscle: 'hamstrings', difficulty: 'intermediate', note: 'Hamstring stretch',
        tutorial: 'Like the Romanian deadlift, but with very little knee bend (legs nearly straight). This puts maximum stretch on the hamstrings. Same rules: hips push back, flat back, dumbbells slide down the front of your legs, stand by driving hips forward. Go only as deep as you can with a flat back.' },
      { id: 'lb3', name: 'Leg Curl', sets: 3, reps: '12-15', unit: 'plates', muscle: 'hamstrings', difficulty: 'beginner', note: 'MWM-989 leg developer',
        tutorial: 'Lie face down (or sit, depending on machine) with the pad against the back of your ankles. Curl your heels toward your butt by squeezing the hamstrings. Pause for 1 second, lower slowly. Pure hamstring isolation — easy on the lower back.' },
      { id: 'lb4', name: 'Goblet Squat', sets: 3, reps: '12-15', unit: 'db', muscle: 'quads', difficulty: 'beginner', note: 'Higher reps today',
        tutorial: 'Same as Lower A — see that tutorial. Today is higher reps with slightly lighter weight, focusing on time under tension.' },
      { id: 'lb5', name: 'Seated Calf Raise', sets: 4, reps: '12-15', unit: 'db', muscle: 'calves', difficulty: 'beginner', note: 'Different head',
        tutorial: 'Sit on the bench, dumbbells resting on your knees (one on each thigh, held in place with your hands). Rise up on your toes as high as you can, squeeze, lower slowly. The bent knee position targets the soleus — a different calf muscle than standing raises hit.' },
      { id: 'lb6', name: 'Cable Crunch', sets: 3, reps: '10-15', unit: 'plates', muscle: 'core', difficulty: 'beginner', note: 'Loaded abs',
        tutorial: 'Kneel in front of the MWM-989 high pulley with a rope or bar. Hold the handle by your head/ears. Crunch DOWN by curling your torso (think rolling your ribs to your hips). Don\'t pull with your arms — they\'re just holding the weight in place. Squeeze abs hard at the bottom.' },
    ],
  },
};

// Equipment-audited swaps: every option works with bench, ≤30lb DBs, MWM-989, or BW.
const SWAPS = {
  chest: [
    { name: 'DB Bench Press', unit: 'db', difficulty: 'beginner', muscle: 'Whole chest (middle and lower)', why: 'The classic chest builder. Hits the whole muscle and lets you load it heavy as you grow.', when: 'Default pick. If you can press dumbbells without shoulder pain, use this.' },
    { name: 'Incline DB Bench Press', unit: 'db', difficulty: 'beginner', muscle: 'Upper chest (the part near your collarbone)', why: 'Builds the top of your chest, which is what fills out a t-shirt at the neckline.', when: 'When your lower chest is catching up but the top still looks flat. Or if regular bench bores you.' },
    { name: 'Machine Chest Press', unit: 'plates', difficulty: 'beginner', muscle: 'Whole chest, fixed path', why: 'Machine guides the movement, so you can\'t mess up the form. Easier on shoulders than free weights.', when: 'Shoulder feels tweaky. Or your last session was so hard your stabilizer muscles are toast.' },
    { name: 'Pec Fly (cable)', unit: 'plates', difficulty: 'beginner', muscle: 'Inner chest line', why: 'Isolates the chest with no shoulder/tricep help. Cross both MWM cables in front of you. Creates the line down the middle of your chest.', when: 'You\'ve already pressed and want to finish the chest off without more pressing fatigue.' },
    { name: 'Push-Up (Feet Elevated)', unit: 'bw', difficulty: 'beginner', muscle: 'Whole chest + shoulders + triceps', why: 'No equipment needed. Feet on a bench makes it harder than regular push-ups.', when: 'Equipment is in use, or as a warm-up before pressing.' },
  ],
  back: [
    { name: 'Lat Pulldown', unit: 'plates', difficulty: 'beginner', muscle: 'Lats (the wing muscles on the sides of your back)', why: 'Builds back WIDTH — half of the V-taper you want. Easier to learn than pull-ups.', when: 'Default pick. The foundation back exercise.' },
    { name: 'Close-Grip Lat Pulldown', unit: 'plates', difficulty: 'beginner', muscle: 'Lower lats + biceps assist', why: 'Hits the bottom part of the lats more, easier on your shoulders than wide-grip.', when: 'Shoulders feel tight on regular pulldowns. Or you want a different feel from wide-grip.' },
    { name: 'Machine Seated Row', unit: 'plates', difficulty: 'beginner', muscle: 'Mid-back (between your shoulder blades)', why: 'Builds back THICKNESS — depth from the side. Different look than pulldowns build.', when: 'You want a "fuller" back look from the side, or to balance out lots of pulldowns.' },
    { name: 'DB Row (single arm)', unit: 'db', difficulty: 'beginner', muscle: 'One side of your back at a time', why: 'Fixes left/right imbalance. Place a knee on the bench, other foot on floor, row the DB to your hip.', when: 'You notice one side does more work than the other. Or as a finisher.' },
    { name: 'Pull-Up / Chin-Up', unit: 'bw', difficulty: 'advanced', muscle: 'Whole back + biceps', why: 'King of bodyweight back exercises if you have a pull-up bar. Builds insane back density.', when: 'You have a pull-up bar and can do at least 3-5 clean reps.' },
  ],
  shoulders: [
    { name: 'DB Lateral Raise', unit: 'db', difficulty: 'beginner', muscle: 'Side delts (the cap on top of your shoulder that makes shoulders look WIDE)', why: 'The single best exercise for shoulder width. Big visual lever for looking built in a tee.', when: 'Default pick. The shoulder exercise you should never skip.' },
    { name: 'Cable Lateral Raise', unit: 'plates', difficulty: 'beginner', muscle: 'Side delts', why: 'MWM-989 low pulley with D-handle. Cables give constant tension the whole movement — DBs are easy at the bottom and hard at the top.', when: 'Often grows shoulders faster than DBs once you\'ve done both a while.' },
    { name: 'DB Overhead Press', unit: 'db', difficulty: 'intermediate', muscle: 'Front delts + side delts', why: 'Builds strength and overall shoulder size. Compound movement, lots of muscle worked.', when: 'You want to focus on shoulder STRENGTH, not just size. Or to add variety.' },
    { name: 'Lean-Away Cable Lateral', unit: 'plates', difficulty: 'intermediate', muscle: 'Side delts, maximum stretch', why: 'Stand slightly leaned away from the MWM-989 low pulley — the side delt stretches fully at the bottom.', when: 'You\'ve done regular laterals for months and want more growth from the same muscle.' },
  ],
  'rear-delts': [
    { name: 'Face Pull (cable)', unit: 'plates', difficulty: 'beginner', muscle: 'Back of shoulders + upper back (fixes posture)', why: 'Builds the rear shoulder AND counteracts forward shoulders from gaming and desk time. Non-negotiable for posture.', when: 'Default pick. Do these even if you skip everything else.' },
    { name: 'Rear Delt DB Fly', unit: 'db', difficulty: 'beginner', muscle: 'Back of shoulders', why: 'Bend over at the waist, raise dumbbells out to the sides like wings. Same target as face pulls but with DBs.', when: 'Variety. Or warm-up.' },
    { name: 'Bent-Over Cable Rear Fly', unit: 'plates', difficulty: 'intermediate', muscle: 'Back of shoulders, constant tension', why: 'MWM-989 cables crossed in front, bend over, pull apart. Cables stay loaded the whole movement.', when: 'Want a different feel than DB rear flies.' },
  ],
  biceps: [
    { name: 'DB Hammer Curl', unit: 'db', difficulty: 'beginner', muscle: 'Bicep + forearm (palms facing each other)', why: 'Builds the bicep AND forearm. Forearms make your arm look thicker in short sleeves.', when: 'Default pick. Works two muscles for the price of one.' },
    { name: 'DB Incline Curl', unit: 'db', difficulty: 'beginner', muscle: 'Long head of bicep (the peak)', why: 'The stretched position at the bottom is what builds the bicep "peak" you see when flexing.', when: 'You\'ve done hammer curls for a while and want to build the bicep peak specifically.' },
    { name: 'Preacher Curl (MWM pad)', unit: 'plates', difficulty: 'beginner', muscle: 'Lower bicep (forces strict form)', why: 'Use the preacher pad on the MWM-989 attached to the low cable. The pad prevents cheating with momentum.', when: 'You catch yourself swinging the weight on regular curls.' },
    { name: 'DB Concentration Curl', unit: 'db', difficulty: 'beginner', muscle: 'Bicep peak, one arm at a time', why: 'Sit on the bench, elbow braced on inner thigh, curl one arm. Classic bodybuilder finisher.', when: 'End-of-workout finisher, or to fix arm size imbalance.' },
    { name: 'Cable Curl (low pulley)', unit: 'plates', difficulty: 'beginner', muscle: 'Whole bicep, constant tension', why: 'MWM-989 low pulley with bar or rope attachment. Constant tension the whole movement.', when: 'Variety. Often grows arms faster after months of just DB curls.' },
  ],
  triceps: [
    { name: 'DB Overhead Tricep Ext.', unit: 'db', difficulty: 'beginner', muscle: 'Long head of tricep (the back of your arm)', why: 'The long head is the biggest part of the tricep and gives the back of your arm visible size.', when: 'Default pick. Biggest bang-for-buck tricep exercise.' },
    { name: 'DB Skull Crusher', unit: 'db', difficulty: 'intermediate', muscle: 'Whole tricep, mass builder', why: 'Hits all three tricep heads. Builds raw tricep size.', when: 'You want to load triceps heavier than overhead extensions allow.' },
    { name: 'Cable Tricep Pushdown', unit: 'plates', difficulty: 'beginner', muscle: 'Lateral head of tricep (the outer part)', why: 'MWM-989 high pulley with bar or rope. Builds the outer tricep — most visible from the side.', when: 'Variety. Or as a finisher after pressing.' },
    { name: 'Close-Grip DB Press', unit: 'db', difficulty: 'beginner', muscle: 'Triceps + chest', why: 'DB Bench Press with hands closer together. Compound — hits chest and triceps together.', when: 'You want strength + size in one move. Good for short workouts.' },
    { name: 'Diamond Push-Up', unit: 'bw', difficulty: 'intermediate', muscle: 'Triceps + inner chest', why: 'Push-up with hands close together in a diamond shape. No equipment needed.', when: 'Tricep finisher, or no equipment available.' },
  ],
  quads: [
    { name: 'Goblet Squat', unit: 'db', difficulty: 'beginner', muscle: 'Quads (front of thigh) + glutes', why: 'Easiest squat variation to learn. Holding the weight in front forces a good upright posture.', when: 'Default pick until you have heavier DBs.' },
    { name: 'DB Bulgarian Split Squat', unit: 'db', difficulty: 'advanced', muscle: 'Quads + glutes, one leg at a time', why: 'Builds huge legs with light weight because all the load is on one leg. Fixes imbalances.', when: 'Light DBs only — works great. Hard to balance at first, give it 2-3 sessions.' },
    { name: 'DB Walking Lunge', unit: 'db', difficulty: 'beginner', muscle: 'Quads + glutes, moving', why: 'Easier balance than Bulgarian split squats. Glutes get more work because you step forward.', when: 'Want unilateral leg work without the balance challenge of Bulgarians.' },
    { name: 'Leg Extension', unit: 'plates', difficulty: 'beginner', muscle: 'Quads only (pure isolation)', why: 'MWM-989 leg developer. Hits quads without any glute, back, or balance involvement.', when: 'After squats to fully fatigue the quads. Or when you don\'t want to load your spine.' },
    { name: 'DB Step-Up (on bench)', unit: 'db', difficulty: 'beginner', muscle: 'Quads + glutes, unilateral', why: 'Step up onto your bench with DBs. Simple, scalable, great for beginners.', when: 'Beginner-friendly unilateral work. Or as a warm-up.' },
  ],
  hamstrings: [
    { name: 'DB Romanian Deadlift', unit: 'db', difficulty: 'intermediate', muscle: 'Hamstrings + glutes + spinal erectors (low back)', why: 'Best all-around posterior chain builder. Also fixes posture and builds the "athletic back" look.', when: 'Default pick. Do these.' },
    { name: 'DB Stiff-Leg Deadlift', unit: 'db', difficulty: 'intermediate', muscle: 'Hamstrings (max stretch)', why: 'Almost-straight legs put more stretch on the hamstrings, which is what builds size.', when: 'Variation on RDLs. More hamstring, less glute and back.' },
    { name: 'Leg Curl', unit: 'plates', difficulty: 'beginner', muscle: 'Hamstrings only', why: 'MWM-989 leg developer. Pure hamstring isolation — no spinal load, no balance.', when: 'Low back is tired or sore. Or to finish hamstrings after RDLs.' },
    { name: 'Single-Leg RDL', unit: 'db', difficulty: 'advanced', muscle: 'Hamstrings + glutes, one leg + balance', why: 'Hold one DB, hinge on one leg while the other extends straight back. Builds insane balance.', when: 'You\'ve mastered regular RDLs and want a new challenge.' },
    { name: 'Glute Bridge (DB on hips)', unit: 'db', difficulty: 'beginner', muscle: 'Hamstrings + glutes', why: 'Lie on floor, DB on hips, drive hips up. Easy on the back, hits glutes and hamstrings together.', when: 'Low back is sore. Or as a finisher.' },
  ],
  glutes: [
    { name: 'DB Walking Lunge', unit: 'db', difficulty: 'beginner', muscle: 'Glutes + quads, moving', why: 'The walking pattern recruits glutes more than stationary leg exercises.', when: 'Default pick. Easy to learn, hits glutes well.' },
    { name: 'DB Hip Thrust (on bench)', unit: 'db', difficulty: 'beginner', muscle: 'Glutes (max load, isolated)', why: 'Back against the bench, DB on hips, drive hips up. Lets you load glutes heavier than any other exercise.', when: 'Lower back issues — this is back-friendly. Or to grow glutes specifically.' },
    { name: 'DB Bulgarian Split Squat', unit: 'db', difficulty: 'advanced', muscle: 'Glutes + quads, one leg', why: 'Going deep on Bulgarian split squats absolutely smokes the glutes.', when: 'Want both quad AND glute size from one exercise.' },
    { name: 'DB Step-Up', unit: 'db', difficulty: 'beginner', muscle: 'Glutes + quads', why: 'Step onto the bench with DBs. Higher bench = more glute work.', when: 'Beginner-friendly glute work.' },
    { name: 'Cable Glute Kickback', unit: 'plates', difficulty: 'beginner', muscle: 'Glutes only (isolation)', why: 'MWM-989 low pulley with ankle strap (or loop the cable around your foot). Kick one leg straight back.', when: 'Pure glute isolation. Or as a finisher.' },
  ],
  calves: [
    { name: 'Standing Calf Raise', unit: 'db', difficulty: 'beginner', muscle: 'Upper calf (gastrocnemius — the diamond shape)', why: 'Standing position with straight legs targets the bigger, more visible part of the calf.', when: 'Default pick.' },
    { name: 'Seated Calf Raise', unit: 'db', difficulty: 'beginner', muscle: 'Lower calf (soleus — the flat muscle underneath)', why: 'Bent-knee position targets a DIFFERENT calf muscle than standing raises.', when: 'Train both standing and seated for full calf development.' },
    { name: 'Single-Leg Calf Raise', unit: 'bw', difficulty: 'beginner', muscle: 'One calf at a time', why: 'No equipment needed. Bodyweight is plenty for calves.', when: 'No equipment, or to fix imbalance between calves.' },
  ],
  core: [
    { name: 'Hanging Knee Raise', unit: 'bw', difficulty: 'intermediate', muscle: 'Lower abs + grip strength', why: 'Forces lower abs to do the work. Most ab exercises miss the lower part.', when: 'Default pick if you have a pull-up bar.' },
    { name: 'Plank (timed)', unit: 'bw', difficulty: 'beginner', muscle: 'Whole core, isometric', why: 'Builds the bracing strength that protects your back during squats and deadlifts.', when: 'No pull-up bar, or as a warm-up to other core work.' },
    { name: 'Cable Crunch', unit: 'plates', difficulty: 'beginner', muscle: 'Upper abs (six-pack), loadable', why: 'MWM-989 high pulley, kneel facing the machine, crunch down. Lets you ADD WEIGHT to ab work.', when: 'You want six-pack development with actual weight progression.' },
    { name: 'Lying Leg Raise', unit: 'bw', difficulty: 'beginner', muscle: 'Lower abs', why: 'Lie on the floor, raise straight legs to vertical, lower slowly without touching ground.', when: 'No pull-up bar available.' },
  ],
};

// ─── Trophies ──────────────────────────────────────
// Each trophy: id, name, description, category, icon (SVG path data), check function
const TROPHIES = [
  // First-time hits
  { id: 'first_session', name: 'First Session', desc: 'Logged your first exercise', category: 'firsts', icon: 'spark',
    check: (stats) => stats.totalSessions >= 1 },
  { id: 'first_pr', name: 'First PR', desc: 'Set your first personal record', category: 'firsts', icon: 'star',
    check: (stats) => stats.totalPRs >= 1 },
  { id: 'first_brutal', name: 'Hit the Wall', desc: 'Rated a session "Brutal"', category: 'firsts', icon: 'flame',
    check: (stats) => stats.brutalCount >= 1 },
  { id: 'first_swap', name: 'Made It Mine', desc: 'Used the swap feature', category: 'firsts', icon: 'swap',
    check: (stats) => stats.swapCount >= 1 },
  { id: 'all_sets_target', name: 'Full Slate', desc: 'Hit all target sets at target reps in one exercise', category: 'firsts', icon: 'check',
    check: (stats) => stats.fullTargetHits >= 1 },

  // Consistency
  { id: 'sessions_5', name: 'Five Strong', desc: '5 exercises logged', category: 'consistency', icon: 'stack',
    check: (stats) => stats.totalSessions >= 5 },
  { id: 'sessions_10', name: 'Double Digits', desc: '10 exercises logged', category: 'consistency', icon: 'stack',
    check: (stats) => stats.totalSessions >= 10 },
  { id: 'sessions_25', name: 'Quarter Hundred', desc: '25 exercises logged', category: 'consistency', icon: 'stack',
    check: (stats) => stats.totalSessions >= 25 },
  { id: 'sessions_50', name: 'Halfway to a Hundred', desc: '50 exercises logged', category: 'consistency', icon: 'stack',
    check: (stats) => stats.totalSessions >= 50 },
  { id: 'sessions_100', name: 'Century', desc: '100 exercises logged', category: 'consistency', icon: 'crown',
    check: (stats) => stats.totalSessions >= 100 },
  { id: 'first_workout_complete', name: 'Through the Door', desc: 'Finished a full workout', category: 'consistency', icon: 'door',
    check: (stats) => stats.workoutsCompleted >= 1 },
  { id: 'workouts_10', name: 'Showing Up', desc: 'Completed 10 full workouts', category: 'consistency', icon: 'door',
    check: (stats) => stats.workoutsCompleted >= 10 },
  { id: 'all_4_days', name: 'Full Week', desc: 'Trained all 4 days at least once', category: 'consistency', icon: 'four',
    check: (stats) => stats.daysTrained.size >= 4 },

  // Strength milestones
  { id: 'pr_5', name: 'PR Hunter', desc: 'Set 5 personal records', category: 'strength', icon: 'star',
    check: (stats) => stats.totalPRs >= 5 },
  { id: 'pr_10', name: 'On a Roll', desc: 'Set 10 personal records', category: 'strength', icon: 'star',
    check: (stats) => stats.totalPRs >= 10 },
  { id: 'pr_25', name: 'Forged Strength', desc: 'Set 25 personal records', category: 'strength', icon: 'crown',
    check: (stats) => stats.totalPRs >= 25 },
  { id: 'bench_25', name: '25lb Bench Club', desc: 'Bench pressed 25lb DBs for any reps', category: 'strength', icon: 'dumbbell',
    check: (stats) => stats.maxBench >= 25 },
  { id: 'bench_30', name: '30lb Bench Club', desc: 'Bench pressed 30lb DBs for any reps', category: 'strength', icon: 'dumbbell',
    check: (stats) => stats.maxBench >= 30 },
  { id: 'squat_30', name: '30lb Goblet', desc: 'Goblet squatted 30lb for any reps', category: 'strength', icon: 'dumbbell',
    check: (stats) => stats.maxSquat >= 30 },
  { id: 'volume_3000', name: 'Volume King', desc: 'Hit 3000+ lb total volume in one session', category: 'strength', icon: 'bolt',
    check: (stats) => stats.maxSessionVolume >= 3000 },
  { id: 'volume_5000', name: 'Volume Beast', desc: 'Hit 5000+ lb total volume in one session', category: 'strength', icon: 'bolt',
    check: (stats) => stats.maxSessionVolume >= 5000 },
];

// ─── SVG Icons for trophies ────────────────────────
function TrophyIcon({ type, size = 32, color = '#e85d4a' }) {
  const stroke = color;
  const props = { width: size, height: size, viewBox: '0 0 32 32', fill: 'none', stroke, strokeWidth: 1.5, strokeLinecap: 'round', strokeLinejoin: 'round' };
  switch (type) {
    case 'spark':
      return <svg {...props}><path d="M16 4v8M16 20v8M4 16h8M20 16h8M8 8l4 4M20 20l4 4M8 24l4-4M20 12l4-4" /></svg>;
    case 'star':
      return <svg {...props}><path d="M16 4l3.5 7.5L28 13l-6 5.5L23.5 27 16 22.5 8.5 27 10 18.5 4 13l8.5-1.5z" /></svg>;
    case 'flame':
      return <svg {...props}><path d="M16 28c-5 0-9-3.5-9-9 0-4 2-6 4-10 1.5 3 3 5 6 6 0-3 1-5 3-7 1 4 5 7 5 12 0 5-4 8-9 8z" /></svg>;
    case 'swap':
      return <svg {...props}><path d="M7 11h18l-4-4M25 21H7l4 4" /></svg>;
    case 'check':
      return <svg {...props}><circle cx="16" cy="16" r="11" /><path d="M11 16l4 4 6-8" /></svg>;
    case 'stack':
      return <svg {...props}><rect x="6" y="6" width="20" height="4" rx="1" /><rect x="6" y="14" width="20" height="4" rx="1" /><rect x="6" y="22" width="20" height="4" rx="1" /></svg>;
    case 'crown':
      return <svg {...props}><path d="M4 22l3-12 5 6 4-10 4 10 5-6 3 12zM4 26h24" /></svg>;
    case 'door':
      return <svg {...props}><rect x="8" y="4" width="16" height="24" rx="1" /><circle cx="20" cy="16" r="1" fill={stroke} /></svg>;
    case 'four':
      return <svg {...props}><rect x="4" y="4" width="11" height="11" rx="1" /><rect x="17" y="4" width="11" height="11" rx="1" /><rect x="4" y="17" width="11" height="11" rx="1" /><rect x="17" y="17" width="11" height="11" rx="1" /></svg>;
    case 'dumbbell':
      return <svg {...props}><rect x="3" y="12" width="3" height="8" rx="1" /><rect x="26" y="12" width="3" height="8" rx="1" /><rect x="6" y="14" width="3" height="4" /><rect x="23" y="14" width="3" height="4" /><rect x="9" y="15" width="14" height="2" rx="1" /></svg>;
    case 'bolt':
      return <svg {...props}><path d="M18 4L8 18h6l-2 10 10-14h-6z" /></svg>;
    default:
      return <svg {...props}><circle cx="16" cy="16" r="11" /></svg>;
  }
}

const DIFFICULTY_TOOLTIPS = {
  'Easy': 'You could have done 4+ more reps. Time to add weight.',
  'Just right': 'Stopped 1-2 reps shy of failure. Sweet spot for growth.',
  'Hard': 'Last rep was a grinder. Repeat this weight next time, then progress.',
  'Brutal': 'Couldn\'t have done one more rep. Weight might be too heavy — keep it or drop slightly.',
};

function hexToRgba(hex, alpha) {
  const h = hex.replace('#', '');
  const r = parseInt(h.substring(0, 2), 16);
  const g = parseInt(h.substring(2, 4), 16);
  const b = parseInt(h.substring(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function fmtDateShort(iso) {
  if (!iso) return '';
  const d = new Date(iso);
  const diff = Math.floor((Date.now() - d.getTime()) / 86400000);
  if (diff === 0) return 'today';
  if (diff === 1) return 'yesterday';
  if (diff < 7) return `${diff}d ago`;
  if (diff < 30) return `${Math.floor(diff / 7)}w ago`;
  return `${Math.floor(diff / 30)}mo ago`;
}

function fmtMMSS(s) {
  const m = Math.floor(s / 60);
  const sec = s % 60;
  return `${m}:${sec.toString().padStart(2, '0')}`;
}

function youtubeSearchURL(exerciseName) {
  const q = encodeURIComponent(`${exerciseName} proper form tutorial`);
  return `https://www.youtube.com/results?search_query=${q}`;
}

// Detect personal record: weight × reps combo never hit before
function detectPR(exData, newSet, unit) {
  if (!exData.sessions || exData.sessions.length === 0) return false;
  const newWeight = newSet.weight === 'BW' ? 0 : parseFloat(newSet.weight) || 0;
  const newReps = parseInt(newSet.reps) || 0;
  if (newWeight === 0 && unit !== 'bw') return false;
  for (const sess of exData.sessions) {
    for (const s of sess.sets) {
      const w = s.weight === 'BW' ? 0 : parseFloat(s.weight) || 0;
      const r = parseInt(s.reps) || 0;
      if (w >= newWeight && r >= newReps) return false;
    }
  }
  return true;
}

// Compute aggregate stats for trophy checking
function computeStats(data, meta) {
  const stats = {
    totalSessions: meta.totalSessions || 0,
    totalPRs: 0,
    brutalCount: 0,
    swapCount: 0,
    fullTargetHits: 0,
    workoutsCompleted: meta.workoutsCompleted || 0,
    daysTrained: new Set(meta.daysTrained || []),
    maxBench: 0,
    maxSquat: 0,
    maxSessionVolume: meta.maxSessionVolume || 0,
  };

  for (const [exId, exData] of Object.entries(data)) {
    if (exData.swappedTo) stats.swapCount++;
    if (!exData.sessions) continue;

    // Find max bench (DB Bench Press or Incline DB Bench Press, lb)
    if (exId === 'ua1' || exId === 'ub2') {
      for (const sess of exData.sessions) {
        for (const set of sess.sets) {
          const w = parseFloat(set.weight) || 0;
          if (w > stats.maxBench) stats.maxBench = w;
        }
      }
    }
    // Find max goblet squat
    if (exId === 'la1' || exId === 'lb4') {
      for (const sess of exData.sessions) {
        for (const set of sess.sets) {
          const w = parseFloat(set.weight) || 0;
          if (w > stats.maxSquat) stats.maxSquat = w;
        }
      }
    }

    for (const sess of exData.sessions) {
      if (sess.difficultyRating === 'Brutal') stats.brutalCount++;
      if (sess.prHit) stats.totalPRs++;
      if (sess.fullTarget) stats.fullTargetHits++;
    }
  }

  return stats;
}

function checkTrophies(stats, unlocked) {
  const newly = [];
  for (const t of TROPHIES) {
    if (unlocked.includes(t.id)) continue;
    if (t.check(stats)) newly.push(t);
  }
  return newly;
}

// ─── ROOT ──────────────────────────────────────────
export default function WorkoutTracker() {
  const [view, setView] = useState('home');
  const [tab, setTab] = useState('train');
  const [section, setSection] = useState('overview');
  const [gymSubtab, setGymSubtab] = useState('train');
  const [activeDay, setActiveDay] = useState(null);
  const [activeExerciseId, setActiveExerciseId] = useState(null);
  const [modalMode, setModalMode] = useState(null);
  const [data, setData] = useState({});
  const [meta, setMeta] = useState({
    welcomed: false, dayNames: {}, totalSessions: 0, lastDeloadAt: 0,
    dayMoods: [], workoutsCompleted: 0, daysTrained: [], maxSessionVolume: 0,
    unlockedTrophies: [], cardio: [],
  });
  const [loading, setLoading] = useState(true);

  const [pendingDiscard, setPendingDiscard] = useState(null);
  const [hasUnsavedSets, setHasUnsavedSets] = useState(false);
  const [showMoodPrompt, setShowMoodPrompt] = useState(false);

  // Session
  const [sessionStart, setSessionStart] = useState(null);
  const [sessionExercisesLogged, setSessionExercisesLogged] = useState(new Set());
  const [sessionSetsLogged, setSessionSetsLogged] = useState(0);
  const [sessionPRs, setSessionPRs] = useState([]);
  const [sessionSwapped, setSessionSwapped] = useState([]);
  const [sessionSkipped, setSessionSkipped] = useState([]);
  const [sessionVolume, setSessionVolume] = useState(0);
  const [showSummary, setShowSummary] = useState(false);

  // Warmup gate
  const [warmupDone, setWarmupDone] = useState(false);
  const [warmupChecks, setWarmupChecks] = useState([]);

  // PR celebration
  const [lastPR, setLastPR] = useState(null);
  // Trophy celebration
  const [unlockingTrophy, setUnlockingTrophy] = useState(null);

  // Rest timer
  const [restSeconds, setRestSeconds] = useState(0);
  const [restRunning, setRestRunning] = useState(false);
  const [restExpanded, setRestExpanded] = useState(false);
  const restRef = useRef(null);

  useEffect(() => {
    async function load() {
      try {
        const result = await window.storage.get(STORAGE_KEY);
        if (result?.value) {
          const parsed = JSON.parse(result.value);
          setData(parsed.exercises || {});
          setMeta({
            welcomed: parsed.meta?.welcomed || false,
            dayNames: parsed.meta?.dayNames || {},
            totalSessions: parsed.meta?.totalSessions || 0,
            lastDeloadAt: parsed.meta?.lastDeloadAt || 0,
            dayMoods: parsed.meta?.dayMoods || [],
            workoutsCompleted: parsed.meta?.workoutsCompleted || 0,
            daysTrained: parsed.meta?.daysTrained || [],
            maxSessionVolume: parsed.meta?.maxSessionVolume || 0,
            unlockedTrophies: parsed.meta?.unlockedTrophies || [],
            cardio: parsed.meta?.cardio || [],
          });
        }
      } catch (e) {}
      setLoading(false);
    }
    load();
  }, []);

  useEffect(() => {
    if (restRunning) {
      restRef.current = setInterval(() => {
        setRestSeconds(s => {
          if (s >= REST_DEFAULT) {
            setRestRunning(false);
            setTimeout(() => setRestExpanded(false), 800);
            return REST_DEFAULT;
          }
          return s + 1;
        });
      }, 1000);
    } else if (restRef.current) {
      clearInterval(restRef.current);
    }
    return () => restRef.current && clearInterval(restRef.current);
  }, [restRunning]);

  useEffect(() => {
    if (lastPR) {
      const t = setTimeout(() => setLastPR(null), 3000);
      return () => clearTimeout(t);
    }
  }, [lastPR]);

  // Check for trophies after data/meta changes
  function checkAndUnlockTrophies(newData, newMeta) {
    const stats = computeStats(newData, newMeta);
    const newly = checkTrophies(stats, newMeta.unlockedTrophies || []);
    if (newly.length > 0) {
      const updatedMeta = {
        ...newMeta,
        unlockedTrophies: [...(newMeta.unlockedTrophies || []), ...newly.map(t => t.id)],
      };
      // Show one at a time
      setTimeout(() => setUnlockingTrophy(newly[0]), 1500);
      return updatedMeta;
    }
    return newMeta;
  }

  async function persist(nextData, nextMeta, checkTrophy = false) {
    let finalMeta = nextMeta || meta;
    let finalData = nextData || data;
    if (checkTrophy) {
      finalMeta = checkAndUnlockTrophies(finalData, finalMeta);
    }
    setData(finalData);
    setMeta(finalMeta);
    try {
      await window.storage.set(STORAGE_KEY, JSON.stringify({
        exercises: finalData, meta: finalMeta,
      }));
    } catch (e) { console.error('Save failed', e); }
  }

  function getDayName(key) { return meta.dayNames?.[key] || DEFAULT_DAY_NAMES[key]; }
  function getExState(exId) { return data[exId] || { sessions: [], swappedTo: null }; }
  function lastSession(exId) {
    const s = getExState(exId).sessions;
    return s && s.length ? s[s.length - 1] : null;
  }
  function lastWorkoutForDay(dayKey) {
    const day = PROGRAM[dayKey];
    let latest = null;
    for (const ex of day.exercises) {
      const last = lastSession(ex.id);
      if (last && (!latest || new Date(last.date) > new Date(latest))) {
        latest = last.date;
      }
    }
    return latest;
  }

  function startRestTimer() {
    setRestSeconds(0);
    setRestRunning(true);
    setRestExpanded(true);
  }

  function attemptGoHome() {
    if (hasUnsavedSets) {
      setPendingDiscard('home');
    } else if (sessionExercisesLogged.size > 0) {
      setShowSummary(true);
    } else {
      setView('home');
      setSection('gym');
      setGymSubtab('train');
      resetSession();
    }
  }

  function resetSession() {
    setSessionStart(null);
    setSessionExercisesLogged(new Set());
    setSessionSetsLogged(0);
    setSessionPRs([]);
    setSessionSwapped([]);
    setSessionSkipped([]);
    setSessionVolume(0);
    setWarmupDone(false);
    setWarmupChecks([]);
  }

  function confirmDiscard() {
    if (pendingDiscard === 'home') {
      if (sessionExercisesLogged.size > 0) {
        setShowSummary(true);
      } else {
        setView('home');
        setSection('gym');
        setGymSubtab('train');
        resetSession();
      }
    }
    setHasUnsavedSets(false);
    setPendingDiscard(null);
  }

  function endSession() {
    // Record workout completed if any exercises logged
    if (sessionExercisesLogged.size > 0) {
      const newMaxVol = Math.max(meta.maxSessionVolume || 0, sessionVolume);
      const dayList = meta.daysTrained || [];
      const updatedDays = dayList.includes(activeDay) ? dayList : [...dayList, activeDay];
      const newMeta = {
        ...meta,
        workoutsCompleted: (meta.workoutsCompleted || 0) + 1,
        maxSessionVolume: newMaxVol,
        daysTrained: updatedDays,
      };
      persist(data, newMeta, true);
    }
    setShowSummary(false);
    setView('home');
    setSection('gym');
    setGymSubtab('train');
    resetSession();
    setRestRunning(false);
    setRestExpanded(false);
  }

  if (loading) {
    return (
      <div style={{ ...S.app, display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: '60vh' }}>
        <div style={{ color: '#e85d4a', letterSpacing: '0.3em', fontSize: 12 }}>LOADING…</div>
      </div>
    );
  }

  if (!meta.welcomed) {
    return <Welcome onContinue={() => persist(null, { ...meta, welcomed: true })} />;
  }

  const sessionsSinceDeload = meta.totalSessions - (meta.lastDeloadAt || 0);
  const shouldDeload = sessionsSinceDeload >= DELOAD_AFTER_SESSIONS;

  // ─── HOME / OVERVIEW ───────────────────────────────
  if (view === 'home') {
    // Compute overview stats
    const now = Date.now();
    const weekAgo = now - 7 * 86400000;
    let weekWorkouts = 0;
    let weekVolume = 0;
    let lastWorkoutDate = null;
    let lastWorkoutDayKey = null;
    for (const [exId, exData] of Object.entries(data)) {
      if (!exData.sessions) continue;
      const ex = findExercise(exId, PROGRAM);
      for (const sess of exData.sessions) {
        const t = new Date(sess.date).getTime();
        if (!lastWorkoutDate || t > new Date(lastWorkoutDate).getTime()) {
          lastWorkoutDate = sess.date;
          if (ex) {
            for (const [dKey, dDef] of Object.entries(PROGRAM)) {
              if (dDef.exercises.some(e => e.id === exId)) { lastWorkoutDayKey = dKey; break; }
            }
          }
        }
        if (t >= weekAgo) {
          for (const s of sess.sets) {
            const w = s.weight === 'BW' ? 0 : parseFloat(s.weight) || 0;
            const r = parseInt(s.reps) || 0;
            weekVolume += (ex && ex.unit === 'plates' ? w * PLATE_LB : w) * r;
          }
        }
      }
    }
    // count distinct workout-days in last 7 days
    const dayKeysThisWeek = new Set();
    for (const exData of Object.values(data)) {
      if (!exData.sessions) continue;
      for (const sess of exData.sessions) {
        if (new Date(sess.date).getTime() >= weekAgo) {
          dayKeysThisWeek.add(sess.date.slice(0, 10));
        }
      }
    }
    weekWorkouts = dayKeysThisWeek.size;

    const cardioThisWeek = (meta.cardio || []).filter(c => new Date(c.date).getTime() >= weekAgo);
    const cardioMin = cardioThisWeek.reduce((sum, c) => sum + (c.duration || 0), 0);

    const greeting = (() => {
      const h = new Date().getHours();
      if (h < 5) return 'Up late';
      if (h < 12) return 'Morning';
      if (h < 17) return 'Afternoon';
      if (h < 21) return 'Evening';
      return 'Up late';
    })();
    const dateStr = new Date().toLocaleDateString('en-US', { weekday: 'long', month: 'long', day: 'numeric' });

    // OVERVIEW SCREEN
    if (section === 'overview') {
      return (
        <div style={S.app}>
          <style>{globalCSS}</style>
          <div style={S.brandBar}>
            <span style={S.brandMark}>◆ FORGE</span>
            <button style={S.exportBtn} onClick={() => setModalMode('export')}>
              <FileDown size={13} /> export
            </button>
          </div>

          <div style={S.fadeIn}>
            <div style={{ marginBottom: 20 }}>
              <div style={S.overviewDate}>{dateStr.toUpperCase()}</div>
              <h1 style={S.overviewGreeting}>{greeting}.</h1>
            </div>

            {shouldDeload && (
              <div style={S.deloadBanner} onClick={() => setModalMode('deload')}>
                <Flame size={16} />
                <div style={{ flex: 1 }}>
                  <div style={{ fontWeight: 700, fontSize: 12, letterSpacing: '0.05em' }}>Deload recommended</div>
                  <div style={{ fontSize: 11, color: '#a89580', marginTop: 2 }}>{sessionsSinceDeload} sessions since last reset. Tap to read why.</div>
                </div>
              </div>
            )}

            {/* This week strip */}
            <div style={S.weekStrip}>
              <div style={S.weekStripLabel}>THIS WEEK</div>
              <div style={S.weekStripRow}>
                <div style={S.weekStat}>
                  <div style={S.weekStatVal}>{weekWorkouts}</div>
                  <div style={S.weekStatLabel}>WORKOUTS</div>
                </div>
                <div style={S.weekStripDivider} />
                <div style={S.weekStat}>
                  <div style={S.weekStatVal}>{weekVolume >= 1000 ? `${(weekVolume / 1000).toFixed(1)}k` : weekVolume}</div>
                  <div style={S.weekStatLabel}>LB MOVED</div>
                </div>
                <div style={S.weekStripDivider} />
                <div style={S.weekStat}>
                  <div style={S.weekStatVal}>{cardioMin}</div>
                  <div style={S.weekStatLabel}>CARDIO MIN</div>
                </div>
              </div>
              {lastWorkoutDate && (
                <div style={S.lastSession}>
                  Last: <span style={{ color: '#d4c5a8' }}>{lastWorkoutDayKey ? getDayName(lastWorkoutDayKey) : 'workout'}</span> · {fmtDateShort(lastWorkoutDate)}
                </div>
              )}
            </div>

            {/* Tile grid: Gym big on top, 3 smaller below */}
            <button style={{ ...S.tileBig, ...S.tilePressable }} onClick={() => { setSection('gym'); setGymSubtab('train'); }}>
              <div style={S.tileBgGradient} />
              <div style={S.tileBigInner}>
                <div>
                  <div style={S.tileLabelLg}>GYM</div>
                  <div style={S.tileSubLg}>{Object.keys(PROGRAM).length} days · {meta.totalSessions} exercises logged</div>
                </div>
                <div style={S.tileArrowBig}>→</div>
              </div>
            </button>

            <div style={S.tileRow}>
              <button style={{ ...S.tileSmall, ...S.tilePressable }} onClick={() => setSection('cardio')}>
                <div style={S.tileSmallInner}>
                  <Activity size={18} color="#d4c5a8" style={{ marginBottom: 8 }} />
                  <div style={S.tileLabelSm}>CARDIO</div>
                  <div style={S.tileSubSm}>{cardioMin > 0 ? `${cardioMin} min this wk` : 'no logs yet'}</div>
                </div>
              </button>
              <button style={{ ...S.tileSmall, ...S.tilePressable }} onClick={() => setSection('trophies')}>
                <div style={S.tileSmallInner}>
                  <Trophy size={18} color="#d4c5a8" style={{ marginBottom: 8 }} />
                  <div style={S.tileLabelSm}>TROPHIES</div>
                  <div style={S.tileSubSm}>{(meta.unlockedTrophies || []).length} / {TROPHIES.length}</div>
                </div>
              </button>
            </div>

            <button style={{ ...S.tileSmall, ...S.tileDisabled, width: '100%' }} disabled>
              <div style={S.tileSmallInner}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <div style={S.tileLabelSm}>NUTRITION</div>
                  <div style={S.wipBadge}>WIP</div>
                </div>
                <div style={S.tileSubSm}>coming soon</div>
              </div>
            </button>
          </div>

          {modalMode === 'export' && (
            <ExportModal data={data} meta={meta} program={PROGRAM} getDayName={getDayName} onClose={() => setModalMode(null)} plateLb={PLATE_LB} />
          )}
          {modalMode === 'deload' && (
            <DeloadModal onClose={() => setModalMode(null)} onMarkDone={() => {
              persist(null, { ...meta, lastDeloadAt: meta.totalSessions });
              setModalMode(null);
            }} />
          )}
          {unlockingTrophy && (
            <TrophyUnlockModal trophy={unlockingTrophy} onClose={() => setUnlockingTrophy(null)} />
          )}
        </div>
      );
    }

    // GYM SECTION
    if (section === 'gym') {
      return (
        <div style={S.app}>
          <style>{globalCSS}</style>
          <div style={S.brandBar}>
            <button style={S.backLink} onClick={() => setSection('overview')}>
              <ChevronLeft size={14} /> overview
            </button>
            <button style={S.exportBtn} onClick={() => setModalMode('export')}>
              <FileDown size={13} /> export
            </button>
          </div>

          <div style={S.fadeIn}>
            <div style={S.sectionHeader}>
              <h1 style={S.sectionTitle}>GYM</h1>
              <div style={S.sectionSub}>{meta.totalSessions} exercises logged</div>
            </div>

            {/* Gym subtabs */}
            <div style={S.subtabRow}>
              <button
                style={{ ...S.subtab, color: gymSubtab === 'train' ? '#f5ead9' : '#6e5d4d', borderBottomColor: gymSubtab === 'train' ? '#e85d4a' : 'transparent' }}
                onClick={() => setGymSubtab('train')}
              >Train</button>
              <button
                style={{ ...S.subtab, color: gymSubtab === 'stats' ? '#f5ead9' : '#6e5d4d', borderBottomColor: gymSubtab === 'stats' ? '#e85d4a' : 'transparent' }}
                onClick={() => setGymSubtab('stats')}
              >Stats</button>
            </div>

            {gymSubtab === 'train' && (
              <>
                {shouldDeload && (
                  <div style={S.deloadBanner} onClick={() => setModalMode('deload')}>
                    <Flame size={16} />
                    <div style={{ flex: 1 }}>
                      <div style={{ fontWeight: 700, fontSize: 12, letterSpacing: '0.05em' }}>Deload recommended</div>
                      <div style={{ fontSize: 11, color: '#a89580', marginTop: 2 }}>{sessionsSinceDeload} sessions since last reset.</div>
                    </div>
                  </div>
                )}

                <p style={{ ...S.sectionLead, marginBottom: 16 }}>Pick your day.</p>

                <div style={S.dayList}>
                  {(() => {
                    // Compute which day is "next up" — oldest lastDate, or first if no history
                    const entries = Object.entries(PROGRAM);
                    let nextUpKey = entries[0][0];
                    let oldestTime = Infinity;
                    let anyLogged = false;
                    for (const [k] of entries) {
                      const ld = lastWorkoutForDay(k);
                      if (ld) {
                        anyLogged = true;
                        const t = new Date(ld).getTime();
                        if (t < oldestTime) { oldestTime = t; nextUpKey = k; }
                      } else if (!anyLogged) {
                        // No-one logged yet, oldest = none, take the first one without log
                        nextUpKey = k;
                        break;
                      }
                    }
                    // If some are logged and some aren't, the unlogged one is most overdue
                    if (anyLogged) {
                      for (const [k] of entries) {
                        if (!lastWorkoutForDay(k)) { nextUpKey = k; break; }
                      }
                    }

                    return entries
                      .slice()
                      .sort((a, b) => (a[0] === nextUpKey ? -1 : b[0] === nextUpKey ? 1 : 0))
                      .map(([key, dayDef]) => {
                      const lastDate = lastWorkoutForDay(key);
                      const isToday = key === nextUpKey;
                      return (
                        <DayCard
                          key={key}
                          dayKey={key}
                          day={dayDef}
                          name={getDayName(key)}
                          defaultName={DEFAULT_DAY_NAMES[key]}
                          lastDate={lastDate}
                          isToday={isToday}
                          onOpen={() => {
                            setActiveDay(key);
                            setView('day');
                            setSessionStart(Date.now());
                            setSessionExercisesLogged(new Set());
                            setSessionSetsLogged(0);
                            setSessionPRs([]);
                            setSessionSwapped([]);
                            setSessionSkipped([]);
                            setSessionVolume(0);
                            setWarmupDone(false);
                            setWarmupChecks(new Array(dayDef.warmup.length).fill(false));
                          }}
                          onRename={(newName) => {
                            const nextNames = { ...meta.dayNames, [key]: newName };
                            persist(null, { ...meta, dayNames: nextNames });
                          }}
                          onResetName={() => {
                            const nextNames = { ...meta.dayNames };
                            delete nextNames[key];
                            persist(null, { ...meta, dayNames: nextNames });
                          }}
                        />
                      );
                    });
                  })()}
                </div>

                <div style={S.tipBlock}>
                  <div style={S.tipLabel}>The rule</div>
                  <div style={S.tipText}>Add one rep or a touch of weight every session. Stop 1-2 reps shy of failure. Write it down.</div>
                </div>
              </>
            )}

            {gymSubtab === 'stats' && (
              <AnalyticsView data={data} meta={meta} program={PROGRAM} getDayName={getDayName} plateLb={PLATE_LB} />
            )}
          </div>

          {modalMode === 'export' && (
            <ExportModal data={data} meta={meta} program={PROGRAM} getDayName={getDayName} onClose={() => setModalMode(null)} plateLb={PLATE_LB} />
          )}
          {modalMode === 'deload' && (
            <DeloadModal onClose={() => setModalMode(null)} onMarkDone={() => {
              persist(null, { ...meta, lastDeloadAt: meta.totalSessions });
              setModalMode(null);
            }} />
          )}
          {unlockingTrophy && (
            <TrophyUnlockModal trophy={unlockingTrophy} onClose={() => setUnlockingTrophy(null)} />
          )}
        </div>
      );
    }

    // CARDIO SECTION
    if (section === 'cardio') {
      return (
        <div style={S.app}>
          <style>{globalCSS}</style>
          <div style={S.brandBar}>
            <button style={S.backLink} onClick={() => setSection('overview')}>
              <ChevronLeft size={14} /> overview
            </button>
            <button style={S.exportBtn} onClick={() => setModalMode('export')}>
              <FileDown size={13} /> export
            </button>
          </div>

          <div style={S.fadeIn}>
            <CardioView meta={meta}
              onLog={(entry) => {
                const newCardio = [...(meta.cardio || []), entry].slice(-100);
                persist(null, { ...meta, cardio: newCardio });
              }}
              onUpdate={(idx, entry) => {
                const newCardio = [...(meta.cardio || [])];
                newCardio[idx] = entry;
                persist(null, { ...meta, cardio: newCardio });
              }}
              onDelete={(idx) => {
                const newCardio = [...(meta.cardio || [])];
                newCardio.splice(idx, 1);
                persist(null, { ...meta, cardio: newCardio });
              }}
            />
          </div>

          {modalMode === 'export' && (
            <ExportModal data={data} meta={meta} program={PROGRAM} getDayName={getDayName} onClose={() => setModalMode(null)} plateLb={PLATE_LB} />
          )}
          {unlockingTrophy && (
            <TrophyUnlockModal trophy={unlockingTrophy} onClose={() => setUnlockingTrophy(null)} />
          )}
        </div>
      );
    }

    // TROPHIES SECTION
    if (section === 'trophies') {
      return (
        <div style={S.app}>
          <style>{globalCSS}</style>
          <div style={S.brandBar}>
            <button style={S.backLink} onClick={() => setSection('overview')}>
              <ChevronLeft size={14} /> overview
            </button>
          </div>

          <div style={S.fadeIn}>
            <TrophiesView unlocked={meta.unlockedTrophies || []} stats={computeStats(data, meta)} />
          </div>

          {unlockingTrophy && (
            <TrophyUnlockModal trophy={unlockingTrophy} onClose={() => setUnlockingTrophy(null)} />
          )}
        </div>
      );
    }
    // Defensive: if section value is unknown, just render nothing (back button will appear elsewhere)
    return null;
  }

  const day = PROGRAM[activeDay];

  // ─── WARMUP GATE ─────────────────────────────────
  if (!warmupDone) {
    return (
      <div style={S.app}>
        <div style={S.brandBar}>
          <button style={S.backLink} onClick={() => { setView('home'); setSection('gym'); setGymSubtab('train'); resetSession(); }}>
            <ChevronLeft size={14} /> all days
          </button>
        </div>

        <div style={{ ...S.dayHeader, borderLeft: `3px solid ${day.color}` }}>
          <h1 style={S.dayTitle}>{getDayName(activeDay)}</h1>
          <p style={S.daySub}>{day.subtitle}</p>
        </div>

        <div style={S.warmupGateCard}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 18 }}>
            <Zap size={20} color={day.color} />
            <div>
              <div style={{ fontSize: 10, letterSpacing: '0.3em', color: day.color, fontWeight: 700 }}>5-MIN WARMUP</div>
              <div style={{ fontSize: 13, color: '#a89580', marginTop: 2 }}>Skipping this is how people get hurt.</div>
            </div>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            {day.warmup.map((it, i) => (
              <button
                key={i}
                style={{ ...S.warmupCheckRow, borderColor: warmupChecks[i] ? day.color : '#3a2d1f' }}
                onClick={() => {
                  const next = [...warmupChecks];
                  next[i] = !next[i];
                  setWarmupChecks(next);
                }}
              >
                <div style={{ ...S.warmupCheckBox, borderColor: warmupChecks[i] ? day.color : '#6e5d4d', background: warmupChecks[i] ? day.color : 'transparent' }}>
                  {warmupChecks[i] && <Check size={12} color="#0a0a0a" strokeWidth={3} />}
                </div>
                <span style={{ flex: 1, fontSize: 13, color: warmupChecks[i] ? '#a89580' : '#f5ead9', textDecoration: warmupChecks[i] ? 'line-through' : 'none' }}>{it}</span>
              </button>
            ))}
          </div>

          <button
            style={{
              ...S.warmupContinueBtn,
              background: warmupChecks.every(c => c) ? day.color : '#2e2419',
              color: warmupChecks.every(c => c) ? '#1a1410' : '#6e5d4d',
              cursor: warmupChecks.every(c => c) ? 'pointer' : 'not-allowed',
            }}
            onClick={() => warmupChecks.every(c => c) && setWarmupDone(true)}
            disabled={!warmupChecks.every(c => c)}
          >
            {warmupChecks.every(c => c) ? 'start workout →' : `check all to continue (${warmupChecks.filter(c => c).length}/${warmupChecks.length})`}
          </button>

          <button style={S.warmupSkipBtn} onClick={() => setWarmupDone(true)}>
            skip warmup (not recommended)
          </button>
        </div>
      </div>
    );
  }

  // ─── DAY VIEW (post-warmup) ──────────────────────
  return (
    <div style={S.app}>
      <div style={S.brandBar}>
        <button style={S.backLink} onClick={attemptGoHome}>
          <ChevronLeft size={14} /> all days
        </button>
        {!restExpanded && (
          <button
            style={{
              ...S.restMini,
              background: restRunning ? day.color : 'transparent',
              color: restRunning ? '#1a1410' : '#a89580',
              borderColor: restRunning ? day.color : '#4a3a28',
            }}
            onClick={() => {
              if (restRunning) { setRestExpanded(true); }
              else { startRestTimer(); }
            }}
          >
            {restRunning ? <Pause size={12} /> : <Play size={12} />}
            {restRunning ? fmtMMSS(REST_DEFAULT - restSeconds) : '2:30 rest'}
          </button>
        )}
      </div>

      <div style={{ ...S.dayHeader, borderLeft: `3px solid ${day.color}` }}>
        <h1 style={S.dayTitle}>{getDayName(activeDay)}</h1>
        <p style={S.daySub}>{day.subtitle}</p>
      </div>

      <div style={S.exList}>
        {day.exercises.map((ex, idx) => (
          <ExerciseCard
            key={ex.id}
            exercise={ex}
            idx={idx}
            state={getExState(ex.id)}
            lastSess={lastSession(ex.id)}
            color={day.color}
            plateLb={PLATE_LB}
            onUnsavedChange={setHasUnsavedSets}
            onLogSet={(sets, note, difficultyRating, prHits, fullTarget, volume) => {
              const next = { ...data };
              const cur = getExState(ex.id);
              next[ex.id] = {
                ...cur,
                sessions: [...(cur.sessions || []), {
                  date: new Date().toISOString(), sets, note: note || '',
                  difficultyRating, prHit: prHits.length > 0, fullTarget,
                }].slice(-50),
              };
              const newMeta = { ...meta, totalSessions: meta.totalSessions + 1 };
              persist(next, newMeta, true);
              setHasUnsavedSets(false);
              setSessionExercisesLogged(new Set([...sessionExercisesLogged, ex.id]));
              setSessionSetsLogged(sessionSetsLogged + sets.length);
              setSessionVolume(sessionVolume + volume);
              if (prHits.length > 0) {
                setSessionPRs([...sessionPRs, ...prHits]);
                setLastPR(prHits[0]);
              }
              if (cur.swappedTo && !sessionSwapped.includes(ex.id)) {
                setSessionSwapped([...sessionSwapped, ex.id]);
              }
              if (Math.random() < 0.25) setShowMoodPrompt(true);
            }}
            onLogSingleSet={() => { startRestTimer(); }}
            onOpenSwap={() => { setActiveExerciseId(ex.id); setModalMode('swap'); }}
            onResetSwap={() => {
              const next = { ...data };
              const cur = getExState(ex.id);
              next[ex.id] = { ...cur, swappedTo: null };
              persist(next);
            }}
            onOpenTutorial={() => { setActiveExerciseId(ex.id); setModalMode('tutorial'); }}
            onSkip={() => {
              setSessionSkipped([...sessionSkipped, ex.id]);
            }}
            onUnskip={() => {
              setSessionSkipped(sessionSkipped.filter(id => id !== ex.id));
            }}
            isSkipped={sessionSkipped.includes(ex.id)}
          />
        ))}
      </div>

      <button style={{ ...S.endWorkoutBtn, borderColor: sessionExercisesLogged.size > 0 ? day.color : '#4a3a28', color: sessionExercisesLogged.size > 0 ? day.color : '#a89580' }} onClick={() => {
        if (sessionExercisesLogged.size > 0) setShowSummary(true);
        else { setView('home'); setSection('gym'); setGymSubtab('train'); }
      }}>
        {sessionExercisesLogged.size > 0 ? 'End workout' : 'Done for today'}
      </button>

      <div style={S.tipBlock}>
        <div style={S.tipLabel}>Reminder</div>
        <div style={S.tipText}>Log every set. The data IS the recap — export and paste to Claude weekly.</div>
      </div>

      {restExpanded && (
        <RestBubble
          seconds={restSeconds} target={REST_DEFAULT} running={restRunning}
          color={day.color}
          onPause={() => setRestRunning(false)}
          onResume={() => setRestRunning(true)}
          onReset={() => { setRestSeconds(0); setRestRunning(true); }}
          onHide={() => setRestExpanded(false)}
          onSkip={() => { setRestRunning(false); setRestSeconds(0); setRestExpanded(false); }}
        />
      )}

      {lastPR && <PRToast pr={lastPR} color={day.color} onDismiss={() => setLastPR(null)} />}

      {showSummary && (
        <SummaryModal
          exercisesLogged={sessionExercisesLogged.size}
          setsLogged={sessionSetsLogged}
          duration={sessionStart ? Math.round((Date.now() - sessionStart) / 60000) : 0}
          prs={sessionPRs}
          swapped={sessionSwapped.length}
          skipped={sessionSkipped.length}
          volume={sessionVolume}
          color={day.color}
          dayName={getDayName(activeDay)}
          onClose={endSession}
        />
      )}

      {modalMode === 'swap' && activeExerciseId && (
        <SwapModal
          exercise={PROGRAM[activeDay].exercises.find(e => e.id === activeExerciseId)}
          currentSwap={getExState(activeExerciseId).swappedTo}
          color={day.color}
          onPick={(swap) => {
            const next = { ...data };
            const cur = getExState(activeExerciseId);
            next[activeExerciseId] = { ...cur, swappedTo: swap };
            persist(next);
            setModalMode(null); setActiveExerciseId(null);
          }}
          onResetDefault={() => {
            const next = { ...data };
            const cur = getExState(activeExerciseId);
            next[activeExerciseId] = { ...cur, swappedTo: null };
            persist(next);
            setModalMode(null); setActiveExerciseId(null);
          }}
          onClose={() => { setModalMode(null); setActiveExerciseId(null); }}
        />
      )}

      {modalMode === 'tutorial' && activeExerciseId && (() => {
        const ex = PROGRAM[activeDay].exercises.find(e => e.id === activeExerciseId);
        const swap = getExState(activeExerciseId).swappedTo;
        const name = swap?.name || ex.name;
        return (
          <TutorialModal name={name} tutorial={ex.tutorial} isSwapped={!!swap}
            onClose={() => { setModalMode(null); setActiveExerciseId(null); }} />
        );
      })()}

      {pendingDiscard && (
        <ConfirmModal
          title="Discard logged sets?"
          body="You have sets entered but not saved. Going back will lose them."
          confirmText="Discard"
          onConfirm={confirmDiscard}
          onCancel={() => setPendingDiscard(null)}
        />
      )}

      {showMoodPrompt && (
        <MoodModal
          onPick={(mood) => {
            const newMoods = [...(meta.dayMoods || []), { date: new Date().toISOString(), mood, day: activeDay }].slice(-100);
            persist(null, { ...meta, dayMoods: newMoods });
            setShowMoodPrompt(false);
          }}
          onSkip={() => setShowMoodPrompt(false)}
        />
      )}

      {unlockingTrophy && (
        <TrophyUnlockModal trophy={unlockingTrophy} onClose={() => setUnlockingTrophy(null)} />
      )}
    </div>
  );
}

// ─── Welcome ───────────────────────────────────────
function Welcome({ onContinue }) {
  return (
    <div style={S.app}>
      <div style={S.brandBar}><span style={S.brandMark}>◆ FORGE</span></div>
      <h1 style={{ ...S.h1, fontSize: 36, marginBottom: 4 }}>Welcome.</h1>
      <p style={{ fontSize: 14, color: '#a89580', marginBottom: 28, lineHeight: 1.6 }}>
        This is your training log. Built for you, with your equipment, your starting point, your goals.
      </p>
      <div style={S.welcomeBlock}>
        {[
          { n: '01', t: '4 days, 4 workouts', b: 'Upper / Lower split. Ramp up from 2-3 days to 4-5 as you build the habit.' },
          { n: '02', t: 'Log every set', b: 'Weight, reps, how hard it felt. The data is the whole point.' },
          { n: '03', t: 'Tap (i) on any exercise', b: 'Plain-language tutorial: setup, the movement, common mistakes.' },
          { n: '04', t: 'Swap freely', b: 'Every exercise has 3-5 variants. Pick what fits the day.' },
          { n: '05', t: 'Export weekly, paste to Claude', b: 'I read the export and tell you what\'s working and what to push.' },
        ].map((s, i) => (
          <div key={i} style={S.welcomeStep}>
            <span style={S.welcomeNum}>{s.n}</span>
            <div><div style={S.welcomeStepTitle}>{s.t}</div><div style={S.welcomeStepBody}>{s.b}</div></div>
          </div>
        ))}
      </div>
      <button style={S.welcomeCTA} onClick={onContinue}>let's go <span style={{ marginLeft: 6 }}>→</span></button>
      <p style={{ fontSize: 10, color: '#6e5d4d', marginTop: 16, textAlign: 'center', letterSpacing: '0.1em' }}>
        START SMALL · BE CONSISTENT · TRUST THE PROCESS
      </p>
    </div>
  );
}

// ─── Tab switch ────────────────────────────────────
function DayCard({ day, dayKey, name, defaultName, lastDate, isToday, onOpen, onRename, onResetName }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(name);
  const isCustom = name !== defaultName;
  const color = day.color;
  const word = day.word || '';
  const exerciseCount = day.exercises.length;

  // Card-tinted background (a hint of day color on warm-brown base)
  const cardBg = `linear-gradient(135deg, ${hexToRgba(color, 0.08)}, ${hexToRgba(color, 0.02)}), linear-gradient(135deg, #241b15 0%, #1f1812 100%)`;
  const cardBorder = hexToRgba(color, 0.22);

  // Spine: translucent gradient pill on the left holding the rotated word
  const spineBg = `linear-gradient(180deg, ${hexToRgba(color, 0.22)} 0%, ${hexToRgba(color, 0.08)} 100%)`;
  const spineBorder = hexToRgba(color, 0.3);
  const spineGlow = `radial-gradient(circle at center, ${hexToRgba(color, 0.18)} 0%, transparent 70%)`;

  // Top-right card glow
  const cornerGlow = `radial-gradient(circle at top right, ${hexToRgba(color, 0.16)} 0%, transparent 70%)`;

  if (isToday) {
    return (
      <div style={{
        position: 'relative', background: cardBg, borderRadius: 16, overflow: 'hidden',
        padding: 14, border: `1px solid ${cardBorder}`,
        display: 'flex', alignItems: 'stretch', gap: 12,
        boxShadow: `0 6px 20px rgba(0,0,0,0.35), 0 0 0 1px ${hexToRgba(color, 0.08)} inset`,
      }}>
        {/* Spine */}
        <div style={{
          position: 'relative', background: spineBg, borderRadius: 10,
          padding: '10px 7px', display: 'flex', alignItems: 'center', justifyContent: 'center',
          minWidth: 42, border: `1px solid ${spineBorder}`, overflow: 'hidden',
        }}>
          <div style={{ position: 'absolute', inset: 0, background: spineGlow, pointerEvents: 'none' }} />
          <div style={{
            position: 'relative', fontFamily: "'Bebas Neue', sans-serif", fontSize: 18,
            color, writingMode: 'vertical-rl', transform: 'rotate(180deg)',
            letterSpacing: '0.18em', fontWeight: 700,
          }}>{word}</div>
        </div>

        {/* Content */}
        <div style={{ position: 'relative', flex: 1, minWidth: 0 }}>
          <div style={{ position: 'absolute', top: 0, right: 0, width: 90, height: 90, background: cornerGlow, pointerEvents: 'none' }} />
          <div style={{ position: 'relative' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 5, marginBottom: 4 }}>
              <div style={{ width: 5, height: 5, borderRadius: '50%', background: color, boxShadow: `0 0 8px ${color}` }} />
              <div style={{ fontSize: 9, letterSpacing: '0.22em', color, fontWeight: 700 }}>NEXT UP</div>
            </div>
            {editing ? (
              <div style={{ display: 'flex', gap: 6, marginBottom: 4 }}>
                <input autoFocus value={draft} onChange={(e) => setDraft(e.target.value)} style={S.inlineEdit}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') { if (draft.trim()) onRename(draft.trim()); setEditing(false); }
                    if (e.key === 'Escape') { setDraft(name); setEditing(false); }
                  }} />
                <button style={S.iconBtnSmall} onClick={() => { if (draft.trim()) onRename(draft.trim()); setEditing(false); }}><Check size={12} /></button>
                {isCustom && (
                  <button style={S.iconBtnSmall} onClick={() => { onResetName(); setDraft(defaultName); setEditing(false); }}><RotateCcw size={12} /></button>
                )}
              </div>
            ) : (
              <button style={{ ...S.dayNameBtn, marginBottom: 4 }} onClick={() => { setDraft(name); setEditing(true); }}>
                <span style={{ fontFamily: "'Bebas Neue', sans-serif", fontSize: 18, color: '#f5ead9', letterSpacing: '0.02em', lineHeight: 1.1 }}>{name}</span>
                <Edit2 size={9} style={{ marginLeft: 6, color: '#6e5d4d' }} />
              </button>
            )}
            <div style={{ fontSize: 11, color: '#a89580', marginBottom: 12 }}>
              {day.subtitle} · {exerciseCount} exercises
            </div>
            <button onClick={onOpen} style={{
              background: color, color: '#1a1410', border: 'none',
              padding: '8px 14px', borderRadius: 8, fontFamily: 'inherit',
              fontSize: 11, fontWeight: 700, letterSpacing: '0.08em',
              cursor: 'pointer', display: 'inline-flex', alignItems: 'center', gap: 4,
            }}>START →</button>
          </div>
        </div>
      </div>
    );
  }

  // Compact version (non-today)
  return (
    <div style={{
      position: 'relative', background: cardBg, borderRadius: 14, overflow: 'hidden',
      padding: 11, border: `1px solid ${cardBorder}`,
      display: 'flex', alignItems: 'stretch', gap: 10,
      boxShadow: '0 3px 10px rgba(0,0,0,0.2)',
    }}>
      <div style={{
        position: 'relative', background: spineBg, borderRadius: 8,
        padding: '8px 6px', display: 'flex', alignItems: 'center', justifyContent: 'center',
        minWidth: 32, border: `1px solid ${spineBorder}`, overflow: 'hidden',
      }}>
        <div style={{ position: 'absolute', inset: 0, background: spineGlow, pointerEvents: 'none' }} />
        <div style={{
          position: 'relative', fontFamily: "'Bebas Neue', sans-serif", fontSize: 14,
          color, writingMode: 'vertical-rl', transform: 'rotate(180deg)',
          letterSpacing: '0.18em', fontWeight: 700,
        }}>{word}</div>
      </div>

      <div style={{ position: 'relative', flex: 1, display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 8, minWidth: 0 }}>
        <div style={{ position: 'absolute', top: 0, right: 0, width: 70, height: 70, background: cornerGlow, pointerEvents: 'none' }} />
        <div style={{ position: 'relative', flex: 1, minWidth: 0 }}>
          {editing ? (
            <div style={{ display: 'flex', gap: 4 }}>
              <input autoFocus value={draft} onChange={(e) => setDraft(e.target.value)} style={{ ...S.inlineEdit, fontSize: 13, padding: '6px 8px' }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') { if (draft.trim()) onRename(draft.trim()); setEditing(false); }
                  if (e.key === 'Escape') { setDraft(name); setEditing(false); }
                }} />
              <button style={{ ...S.iconBtnSmall, width: 24, height: 24 }} onClick={() => { if (draft.trim()) onRename(draft.trim()); setEditing(false); }}><Check size={11} /></button>
              {isCustom && (
                <button style={{ ...S.iconBtnSmall, width: 24, height: 24 }} onClick={() => { onResetName(); setDraft(defaultName); setEditing(false); }}><RotateCcw size={11} /></button>
              )}
            </div>
          ) : (
            <>
              <button style={{ ...S.dayNameBtn, marginBottom: 2 }} onClick={() => { setDraft(name); setEditing(true); }}>
                <span style={{ fontFamily: "'Bebas Neue', sans-serif", fontSize: 15, color: '#f5ead9', letterSpacing: '0.02em' }}>{name}</span>
                <Edit2 size={8} style={{ marginLeft: 5, color: '#6e5d4d' }} />
              </button>
              <div style={{ fontSize: 10, color: '#a89580' }}>
                {lastDate ? fmtDateShort(lastDate) : 'never'} · {exerciseCount} exercises
              </div>
            </>
          )}
        </div>
        {!editing && (
          <button onClick={onOpen} style={{
            position: 'relative', background: 'transparent', border: 'none',
            cursor: 'pointer', padding: '4px 8px', flexShrink: 0, fontFamily: 'inherit',
          }}>
            <span style={{ color, fontSize: 18, fontWeight: 700 }}>→</span>
          </button>
        )}
      </div>
    </div>
  );
}

// ─── Exercise Card ─────────────────────────────────
function ExerciseCard({ exercise, idx, state, lastSess, color, plateLb, onLogSet, onLogSingleSet, onOpenSwap, onResetSwap, onOpenTutorial, onSkip, onUnskip, isSkipped, onUnsavedChange }) {
  const [expanded, setExpanded] = useState(false);
  const [showNote, setShowNote] = useState(false);
  const [showDiffTooltip, setShowDiffTooltip] = useState(null);
  const [noteDraft, setNoteDraft] = useState('');
  const [setsLogged, setSetsLogged] = useState([]);
  const [curWeight, setCurWeight] = useState('');
  const [curReps, setCurReps] = useState('');
  const [difficultyRating, setDifficultyRating] = useState(null);
  const [justSaved, setJustSaved] = useState(false);

  const displayName = state.swappedTo ? state.swappedTo.name : exercise.name;
  const displayUnit = state.swappedTo ? state.swappedTo.unit : exercise.unit;
  const displayDiff = state.swappedTo ? null : exercise.difficulty;

  useEffect(() => {
    const has = setsLogged.length > 0;
    if (onUnsavedChange) onUnsavedChange(has);
  }, [setsLogged.length]);

  useEffect(() => {
    if (expanded && lastSess && setsLogged.length === 0 && !curWeight) {
      const last = lastSess.sets[lastSess.sets.length - 1];
      if (last && last.weight !== 'BW') setCurWeight(last.weight);
      // Pre-fill reps from middle of target range
      const m = exercise.reps.match(/(\d+)-(\d+)/);
      if (m) setCurReps(m[1]);
      else if (/^\d+$/.test(exercise.reps)) setCurReps(exercise.reps);
    }
    // Bodyweight: weight auto-set
    if (expanded && displayUnit === 'bw' && !curReps) {
      const m = exercise.reps.match(/(\d+)-(\d+)/);
      if (m) setCurReps(m[1]);
      else if (/^\d+$/.test(exercise.reps)) setCurReps(exercise.reps);
    }
  }, [expanded]);

  const sparkData = (state.sessions || []).slice(-8).map(s => {
    return s.sets.reduce((max, set) => {
      const w = parseFloat(set.weight) || 0;
      return w > max ? w : max;
    }, 0);
  });

  function unitShort(u) { return u === 'db' ? 'lb' : u === 'plates' ? 'plates' : 'reps'; }
  function plateToLb(plates) { const n = parseFloat(plates); return isNaN(n) ? '' : `≈${Math.round(n * plateLb)}lb`; }
  function bumpWeight(delta) {
    const n = parseFloat(curWeight) || 0;
    const next = n + delta;
    setCurWeight(next < 0 ? '0' : next.toString());
  }
  function bumpReps(delta) {
    const n = parseInt(curReps) || 0;
    const next = n + delta;
    setCurReps(next < 0 ? '0' : next.toString());
  }

  function saveSet() {
    if (!canSaveSet) return;
    const newSet = { weight: displayUnit === 'bw' ? 'BW' : curWeight, reps: curReps };
    setSetsLogged([...setsLogged, newSet]);
    // Keep weight, reset reps to suggested
    const m = exercise.reps.match(/(\d+)-(\d+)/);
    setCurReps(m ? m[1] : (exercise.reps.match(/^\d+/) ? exercise.reps.match(/^\d+/)[0] : ''));
    setJustSaved(true);
    setTimeout(() => setJustSaved(false), 1500);
    if (onLogSingleSet) onLogSingleSet();
  }

  function finishExercise() {
    if (!canFinish) return;
    // Calculate volume + PRs + full-target
    let totalVol = 0;
    const prHits = [];
    const curState = state;
    let runningSessions = [...(curState.sessions || [])];
    for (const s of setsLogged) {
      const w = s.weight === 'BW' ? 0 : parseFloat(s.weight) || 0;
      const r = parseInt(s.reps) || 0;
      totalVol += w * r;
      const isPR = detectPR({ sessions: runningSessions }, s, displayUnit);
      if (isPR) prHits.push({ exerciseName: displayName, weight: s.weight, reps: s.reps, unit: displayUnit });
      runningSessions = [...runningSessions, { sets: [s] }];
    }
    // Full target = all sets done AND all reps >= target lower bound
    const m = exercise.reps.match(/(\d+)-?(\d+)?/);
    const lowReps = m ? parseInt(m[1]) : 0;
    const fullTarget = setsLogged.length >= exercise.sets && setsLogged.every(s => parseInt(s.reps) >= lowReps);

    onLogSet(setsLogged, noteDraft, difficultyRating, prHits, fullTarget, totalVol);
    setSetsLogged([]); setCurWeight(''); setCurReps('');
    setNoteDraft(''); setDifficultyRating(null);
    setShowNote(false); setExpanded(false);
  }

  function removeSet(i) { setSetsLogged(setsLogged.filter((_, idx2) => idx2 !== i)); }

  const canSaveSet = curReps && (displayUnit === 'bw' || curWeight);
  const allSetsDone = setsLogged.length >= exercise.sets;
  const canFinish = setsLogged.length > 0 && difficultyRating !== null;

  const diffColor = displayDiff === 'beginner' ? '#5b9279' : displayDiff === 'intermediate' ? '#d4a017' : '#e85d4a';
  const diffLabel = displayDiff === 'beginner' ? 'EASY' : displayDiff === 'intermediate' ? 'MID' : 'HARD';

  if (isSkipped) {
    return (
      <div style={{ ...S.exCard, opacity: 0.4 }}>
        <button style={S.exHeader} onClick={onUnskip}>
          <div style={{ flex: 1, textAlign: 'left' }}>
            <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
              <span style={S.exIdx}>{String(idx + 1).padStart(2, '0')}</span>
              <span style={{ ...S.exName, textDecoration: 'line-through', color: '#a89580' }}>{displayName}</span>
              <span style={{ fontSize: 9, color: '#6e5d4d', letterSpacing: '0.15em' }}>SKIPPED</span>
            </div>
            <div style={{ ...S.exMeta, fontSize: 10, marginTop: 4 }}>tap to un-skip</div>
          </div>
        </button>
      </div>
    );
  }

  return (
    <div style={S.exCard}>
      <button style={S.exHeader} onClick={() => setExpanded(!expanded)}>
        <div style={{ flex: 1, textAlign: 'left', minWidth: 0 }}>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 8, marginBottom: 3, flexWrap: 'wrap' }}>
            <span style={S.exIdx}>{String(idx + 1).padStart(2, '0')}</span>
            <span style={S.exName}>{displayName}</span>
            {displayDiff && (<span style={{ ...S.diffTag, color: diffColor, borderColor: diffColor }}>{diffLabel}</span>)}
            {state.swappedTo && (<span style={{ ...S.swapTag, color }}>swapped</span>)}
          </div>
          <div style={S.exMeta}>
            <span>{exercise.sets} × {exercise.reps}</span>
            <span style={S.dot}>·</span>
            <span style={S.unitTag}>{unitShort(displayUnit)}</span>
            {lastSess && (<><span style={S.dot}>·</span><span>{fmtDateShort(lastSess.date)}</span></>)}
          </div>
          {sparkData.length >= 2 && <Sparkline data={sparkData} color={color} />}
        </div>
        <div style={{ color: '#6e5d4d', display: 'flex', alignItems: 'center', gap: 4 }}>
          <span role="button" onClick={(e) => { e.stopPropagation(); onOpenTutorial(); }} style={S.infoIconBtn}>
            <Info size={14} />
          </span>
          {expanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
        </div>
      </button>

      {expanded && (
        <div style={S.exBody}>
          <div style={S.exNote}>{exercise.note}</div>

          {lastSess && (
            <div style={S.lastRef}>
              <span style={S.lastRefLabel}>LAST · {fmtDateShort(lastSess.date)}</span>
              <div style={{ marginTop: 4, color: '#a89580', fontSize: 12 }}>
                {lastSess.sets.map((s, i) => (
                  <span key={i} style={{ marginRight: 12 }}>
                    {s.weight === 'BW' ? 'BW' : `${s.weight}${displayUnit === 'db' ? 'lb' : displayUnit === 'plates' ? 'p' : ''}`} × {s.reps}
                  </span>
                ))}
              </div>
              {lastSess.difficultyRating && (
                <div style={{ marginTop: 4, color: '#a89580', fontSize: 11 }}>Felt: {lastSess.difficultyRating}</div>
              )}
            </div>
          )}

          {setsLogged.length > 0 && (
            <div style={S.loggedSets}>
              {setsLogged.map((s, i) => (
                <div key={i} style={S.loggedSet}>
                  <Check size={12} color={color} />
                  <span style={{ color: '#6e5d4d', fontSize: 10, letterSpacing: '0.1em' }}>SET {i + 1}</span>
                  <span style={{ color: '#f5ead9', flex: 1, fontSize: 13 }}>
                    {s.weight === 'BW' ? 'BW' : `${s.weight}${displayUnit === 'db' ? 'lb' : displayUnit === 'plates' ? ' plates' : ''}`} × {s.reps}
                    {displayUnit === 'plates' && s.weight !== 'BW' && <span style={{ color: '#6e5d4d', marginLeft: 6, fontSize: 11 }}>{plateToLb(s.weight)}</span>}
                  </span>
                  <button style={S.removeSet} onClick={() => removeSet(i)}>×</button>
                </div>
              ))}
            </div>
          )}

          {!allSetsDone && (
            <>
              <div style={S.setHeader}>
                <span style={{ color, fontWeight: 700, letterSpacing: '0.1em', fontSize: 11 }}>SET {setsLogged.length + 1}</span>
                <span style={{ color: '#6e5d4d', fontSize: 10 }}>{setsLogged.length}/{exercise.sets} done</span>
              </div>

              <div style={S.setInputRow}>
                <div style={S.inputCol}>
                  <label style={S.inputLabel}>
                    {displayUnit === 'db' ? 'WEIGHT (LB)' : displayUnit === 'plates' ? 'PLATES (1-10)' : 'BODYWEIGHT'}
                  </label>
                  <div style={S.inputWithBumps}>
                    <button style={S.bump} onClick={() => bumpWeight(displayUnit === 'plates' ? -1 : -2.5)} disabled={displayUnit === 'bw'}>
                      <Minus size={12} />
                    </button>
                    <input
                      type="text" inputMode="decimal"
                      value={displayUnit === 'bw' ? 'BW' : curWeight}
                      onChange={(e) => setCurWeight(e.target.value)}
                      placeholder={displayUnit === 'plates' ? '5' : '30'}
                      style={S.numInput} disabled={displayUnit === 'bw'}
                    />
                    <button style={S.bump} onClick={() => bumpWeight(displayUnit === 'plates' ? 1 : 2.5)} disabled={displayUnit === 'bw'}>
                      <Plus size={12} />
                    </button>
                  </div>
                  {displayUnit === 'plates' && curWeight && (<div style={S.plateConvert}>{plateToLb(curWeight)}</div>)}
                </div>

                <div style={{ ...S.inputCol, flex: 0.8 }}>
                  <label style={S.inputLabel}>REPS</label>
                  <div style={S.inputWithBumps}>
                    <button style={S.bump} onClick={() => bumpReps(-1)}><Minus size={12} /></button>
                    <input
                      type="text" inputMode="numeric"
                      value={curReps} onChange={(e) => setCurReps(e.target.value)}
                      placeholder="10" style={S.numInput}
                    />
                    <button style={S.bump} onClick={() => bumpReps(1)}><Plus size={12} /></button>
                  </div>
                </div>
              </div>

              <button
                style={{
                  ...S.saveSetBtn,
                  background: canSaveSet ? color : '#2e2419',
                  color: canSaveSet ? '#1a1410' : '#6e5d4d',
                  cursor: canSaveSet ? 'pointer' : 'not-allowed',
                }}
                onClick={saveSet} disabled={!canSaveSet}
              >
                {justSaved ? (<><Check size={14} /> set {setsLogged.length} saved</>) : (<>save set {setsLogged.length + 1}</>)}
              </button>
            </>
          )}

          {allSetsDone && (
            <div style={{ ...S.allDoneNote, color }}>
              All {exercise.sets} sets done. Rate it below to finish ↓
            </div>
          )}

          {setsLogged.length > 0 && (
            <div style={S.ratingBlock}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8 }}>
                <div style={S.ratingLabel}>HOW HARD DID THIS FEEL? <span style={{ color: '#e85d4a' }}>*REQUIRED</span></div>
              </div>
              <div style={S.ratingRow}>
                {['Easy', 'Just right', 'Hard', 'Brutal'].map((r) => (
                  <button
                    key={r}
                    style={{
                      ...S.ratingBtn,
                      background: difficultyRating === r ? color : 'transparent',
                      color: difficultyRating === r ? '#1a1410' : '#a89580',
                      borderColor: difficultyRating === r ? color : '#4a3a28',
                    }}
                    onClick={() => {
                      setDifficultyRating(difficultyRating === r ? null : r);
                      setShowDiffTooltip(showDiffTooltip === r ? null : r);
                    }}
                  >
                    {r}
                  </button>
                ))}
              </div>
              {showDiffTooltip && (
                <div style={S.tooltipBox}>
                  <span style={{ color, fontWeight: 700 }}>{showDiffTooltip}:</span> {DIFFICULTY_TOOLTIPS[showDiffTooltip]}
                </div>
              )}
            </div>
          )}

          {showNote ? (
            <div style={{ marginBottom: 10 }}>
              <textarea value={noteDraft} onChange={(e) => setNoteDraft(e.target.value)}
                placeholder="form, pain, what to fix next time…"
                style={S.noteInput} rows={2} />
            </div>
          ) : (
            setsLogged.length > 0 && (
              <button style={S.smallLink} onClick={() => setShowNote(true)}>
                <StickyNote size={11} /> add note
              </button>
            )
          )}

          <div style={S.actionRow}>
            <button style={S.smallLink} onClick={onOpenSwap}><Repeat size={11} /> swap</button>
            {state.swappedTo && (
              <button style={S.smallLink} onClick={onResetSwap}><RotateCcw size={11} /> reset</button>
            )}
            <button style={S.smallLink} onClick={() => { onSkip(); setExpanded(false); }}>
              <SkipForward size={11} /> skip
            </button>
            <button
              style={{
                ...S.finishBtn,
                background: canFinish ? color : '#2e2419',
                color: canFinish ? '#1a1410' : '#6e5d4d',
                cursor: canFinish ? 'pointer' : 'not-allowed',
                animation: canFinish ? 'finishPulse 1.4s ease-in-out infinite' : 'none',
                boxShadow: canFinish ? `0 0 0 1px ${color}40` : 'none',
              }}
              onClick={finishExercise} disabled={!canFinish}
            >
              <Check size={13} /> finish
            </button>
          </div>
        </div>
      )}

      <style>{`
        @keyframes finishPulse {
          0%, 100% { transform: scale(1); }
          50% { transform: scale(1.03); }
        }
      `}</style>
    </div>
  );
}

function Sparkline({ data, color }) {
  if (data.length < 2) return null;
  const max = Math.max(...data, 1);
  const min = Math.min(...data, 0);
  const range = max - min || 1;
  const width = 90, height = 14;
  const step = width / (data.length - 1);
  const points = data.map((v, i) => `${i * step},${height - ((v - min) / range) * height}`).join(' ');
  return (
    <svg width={width} height={height} style={{ marginTop: 6, display: 'block' }}>
      <polyline points={points} fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" opacity="0.7" />
    </svg>
  );
}

// ─── Rest Bubble ───────────────────────────────────
function RestBubble({ seconds, target, running, color, onPause, onResume, onReset, onHide, onSkip }) {
  const remaining = Math.max(0, target - seconds);
  const progress = Math.min(100, (seconds / target) * 100);
  const done = remaining === 0;
  return (
    <div style={S.restOverlay}>
      <div style={{ ...S.restBubble, animation: 'restPop 0.3s ease-out' }}>
        <button style={S.restHideBtn} onClick={onHide}><Minimize2 size={14} /> hide</button>
        <div style={{ ...S.restLabel, color }}>{done ? 'TIME' : 'REST'}</div>
        <div style={S.restRing}>
          <svg width="220" height="220" viewBox="0 0 220 220">
            <circle cx="110" cy="110" r="100" fill="none" stroke="#1a1a1a" strokeWidth="10" />
            <circle cx="110" cy="110" r="100" fill="none" stroke={done ? '#5b9279' : color} strokeWidth="10"
              strokeLinecap="round" strokeDasharray={`${2 * Math.PI * 100}`}
              strokeDashoffset={`${2 * Math.PI * 100 * (1 - progress / 100)}`}
              transform="rotate(-90 110 110)" style={{ transition: 'stroke-dashoffset 1s linear' }} />
          </svg>
          <div style={S.restTimeText}>
            <div style={{ ...S.restNumbers, color: done ? '#5b9279' : '#f5ead9' }}>{fmtMMSS(remaining)}</div>
            <div style={S.restSub}>{done ? 'next set' : 'remaining'}</div>
          </div>
        </div>
        <div style={S.restControls}>
          {!done && (running ? (
            <button style={{ ...S.restCtrlBtn, borderColor: color, color }} onClick={onPause}><Pause size={14} /> pause</button>
          ) : (
            <button style={{ ...S.restCtrlBtn, borderColor: color, color }} onClick={onResume}><Play size={14} /> resume</button>
          ))}
          <button style={S.restCtrlBtn} onClick={onReset}><RotateCcw size={14} /> reset</button>
          <button style={S.restCtrlBtn} onClick={onSkip}>skip</button>
        </div>
      </div>
      <style>{`@keyframes restPop { 0% { transform: scale(0.3); opacity: 0; } 70% { transform: scale(1.05); } 100% { transform: scale(1); opacity: 1; } }`}</style>
    </div>
  );
}

function PRToast({ pr, color, onDismiss }) {
  return (
    <div style={S.prToast} onClick={onDismiss}>
      <Trophy size={24} color={color} />
      <div style={{ flex: 1 }}>
        <div style={{ fontSize: 13, fontWeight: 700, color: '#f5ead9', letterSpacing: '0.05em' }}>PERSONAL RECORD ★</div>
        <div style={{ fontSize: 11, color: '#a89580', marginTop: 2 }}>
          {pr.exerciseName}: {pr.weight === 'BW' ? 'BW' : `${pr.weight}${pr.unit === 'plates' ? 'p' : 'lb'}`} × {pr.reps}
        </div>
      </div>
    </div>
  );
}

// ─── Reworked Summary ──────────────────────────────
function SummaryModal({ exercisesLogged, setsLogged, duration, prs, swapped, skipped, volume, color, dayName, onClose }) {
  return (
    <div style={S.modalOverlay}>
      <div style={{ ...S.modalBox, maxWidth: 460, paddingBottom: 28 }}>
        <div style={{ textAlign: 'center', padding: '20px 8px 14px' }}>
          <div style={{ fontSize: 10, letterSpacing: '0.3em', color, marginBottom: 8 }}>SESSION COMPLETE</div>
          <div style={{ fontSize: 24, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif", textTransform: 'uppercase', letterSpacing: '0.02em' }}>
            Workout in the bank.
          </div>
          <div style={{ fontSize: 12, color: '#a89580', marginTop: 6 }}>{dayName}</div>
        </div>

        <div style={S.summaryStats}>
          <div style={S.summaryStat}>
            <div style={{ ...S.summaryStatVal, color }}>{exercisesLogged}</div>
            <div style={S.summaryStatLabel}>EXERCISES</div>
          </div>
          <div style={S.summaryStat}>
            <div style={{ ...S.summaryStatVal, color }}>{setsLogged}</div>
            <div style={S.summaryStatLabel}>SETS</div>
          </div>
          <div style={S.summaryStat}>
            <div style={{ ...S.summaryStatVal, color }}>{duration || '—'}</div>
            <div style={S.summaryStatLabel}>MINUTES</div>
          </div>
        </div>

        <div style={{ marginTop: 16, display: 'flex', flexDirection: 'column', gap: 8 }}>
          <div style={S.summaryRow}>
            <span style={S.summaryRowLabel}>Total volume</span>
            <span style={S.summaryRowVal}>{volume.toLocaleString()} lb</span>
          </div>
          <div style={S.summaryRow}>
            <span style={S.summaryRowLabel}>Personal records</span>
            <span style={{ ...S.summaryRowVal, color: prs.length > 0 ? color : '#6e5d4d' }}>
              {prs.length} {prs.length > 0 && <Trophy size={11} style={{ marginLeft: 4, verticalAlign: 'middle' }} />}
            </span>
          </div>
          <div style={S.summaryRow}>
            <span style={S.summaryRowLabel}>Swapped exercises</span>
            <span style={S.summaryRowVal}>{swapped}</span>
          </div>
          <div style={S.summaryRow}>
            <span style={S.summaryRowLabel}>Skipped exercises</span>
            <span style={{ ...S.summaryRowVal, color: skipped > 0 ? '#d4a017' : '#6e5d4d' }}>{skipped}</span>
          </div>
        </div>

        {prs.length > 0 && (
          <div style={{ marginTop: 14, padding: '12px', background: '#1a1410', border: `1px solid ${color}40`, borderRadius: 3 }}>
            <div style={{ fontSize: 10, letterSpacing: '0.2em', color, fontWeight: 700, marginBottom: 8 }}>PRs THIS SESSION</div>
            {prs.map((pr, i) => (
              <div key={i} style={{ fontSize: 11, color: '#a89580', marginBottom: 3 }}>
                ★ {pr.exerciseName}: {pr.weight === 'BW' ? 'BW' : `${pr.weight}${pr.unit === 'plates' ? 'p' : 'lb'}`} × {pr.reps}
              </div>
            ))}
          </div>
        )}

        <button style={{ ...S.copyBtn, background: color, marginTop: 18 }} onClick={onClose}>done</button>
      </div>
    </div>
  );
}

// ─── Modals (unchanged from v4) ───────────────────
function SwapModal({ exercise, currentSwap, color, onPick, onResetDefault, onClose }) {
  const variants = SWAPS[exercise.muscle] || [];
  return (
    <div style={S.modalOverlay} onClick={onClose}>
      <div style={S.modalBox} onClick={(e) => e.stopPropagation()}>
        <div style={S.modalHeader}>
          <div>
            <div style={{ fontSize: 10, letterSpacing: '0.2em', color: '#6e5d4d' }}>SWAP · {exercise.muscle.toUpperCase()}</div>
            <div style={{ fontSize: 18, fontWeight: 700, marginTop: 4 }}>Pick a variant</div>
          </div>
          <button style={S.modalClose} onClick={onClose}><X size={16} /></button>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {variants.map((v, i) => {
            const isDefault = v.name === exercise.name;
            const isActive = currentSwap?.name === v.name || (isDefault && !currentSwap);
            const diffColor = v.difficulty === 'beginner' ? '#5b9279' : v.difficulty === 'intermediate' ? '#d4a017' : '#e85d4a';
            return (
              <button key={i}
                style={{ ...S.swapItem, borderColor: isActive ? color : '#3a2d1f', background: isActive ? '#2e2419' : '#241b15' }}
                onClick={() => isDefault ? onResetDefault() : onPick({ name: v.name, unit: v.unit })}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', gap: 8, marginBottom: 6 }}>
                  <span style={{ fontSize: 14, fontWeight: 600, color: '#f5ead9' }}>{v.name}</span>
                  <div style={{ display: 'flex', gap: 6, flexShrink: 0 }}>
                    <span style={{ fontSize: 9, color: diffColor, letterSpacing: '0.15em', border: `1px solid ${diffColor}`, padding: '2px 5px', borderRadius: 2 }}>
                      {v.difficulty === 'beginner' ? 'EASY' : v.difficulty === 'intermediate' ? 'MID' : 'HARD'}
                    </span>
                    <span style={{ fontSize: 9, color: '#6e5d4d', letterSpacing: '0.15em' }}>
                      {v.unit === 'db' ? 'DB' : v.unit === 'plates' ? 'MACHINE' : 'BW'}
                    </span>
                  </div>
                </div>
                {isDefault && <div style={{ fontSize: 10, color, marginBottom: 6, letterSpacing: '0.1em' }}>★ DEFAULT</div>}
                <div style={S.swapMeta}><div style={S.swapMetaLabel}>HITS</div><div style={S.swapMetaText}>{v.muscle}</div></div>
                <div style={S.swapMeta}><div style={S.swapMetaLabel}>WHY IT WORKS</div><div style={S.swapMetaText}>{v.why}</div></div>
                <div style={S.swapMeta}><div style={S.swapMetaLabel}>WHEN TO PICK</div><div style={S.swapMetaText}>{v.when}</div></div>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function TutorialModal({ name, tutorial, isSwapped, onClose }) {
  return (
    <div style={S.modalOverlay} onClick={onClose}>
      <div style={S.modalBox} onClick={(e) => e.stopPropagation()}>
        <div style={S.modalHeader}>
          <div>
            <div style={{ fontSize: 10, letterSpacing: '0.2em', color: '#6e5d4d' }}>HOW TO DO IT</div>
            <div style={{ fontSize: 18, fontWeight: 700, marginTop: 4 }}>{name}</div>
          </div>
          <button style={S.modalClose} onClick={onClose}><X size={16} /></button>
        </div>
        {isSwapped ? (
          <div style={S.tutorialBox}>
            <div style={{ color: '#a89580', fontSize: 13, lineHeight: 1.6 }}>
              You've swapped this exercise. Tap "Watch demo" below for a video of "{name}" specifically.
            </div>
          </div>
        ) : (
          <div style={S.tutorialBox}>
            <div style={{ color: '#f5ead9', fontSize: 13, lineHeight: 1.7 }}>{tutorial}</div>
          </div>
        )}
        <a href={youtubeSearchURL(name)} target="_blank" rel="noopener noreferrer" style={S.youtubeBtn}>
          <ExternalLink size={14} /> Watch demo on YouTube
        </a>
        <div style={{ ...S.tipText, marginTop: 16, textAlign: 'center', fontSize: 10 }}>Form &gt; weight. Always.</div>
      </div>
    </div>
  );
}

function ExportModal({ data, meta, program, getDayName, onClose, plateLb }) {
  const [scope, setScope] = useState('7');
  const [copied, setCopied] = useState(false);
  function generate() {
    const cutoff = scope === 'all' ? 0 : Date.now() - parseInt(scope) * 86400000;
    const lines = [];
    lines.push(`FORGE — Workout log`);
    lines.push(`Scope: last ${scope === 'all' ? 'all time' : scope + ' days'}`);
    lines.push(`Generated: ${new Date().toISOString().slice(0, 10)}`);
    lines.push(`Total sessions ever: ${meta.totalSessions}`);
    lines.push(`Workouts completed: ${meta.workoutsCompleted || 0}`);
    lines.push(`Trophies unlocked: ${(meta.unlockedTrophies || []).length}/${TROPHIES.length}`);
    lines.push('─'.repeat(40));
    for (const [dayKey, day] of Object.entries(program)) {
      const dayEntries = [];
      for (const ex of day.exercises) {
        const exData = data[ex.id];
        if (!exData?.sessions) continue;
        const recent = exData.sessions.filter(s => new Date(s.date).getTime() >= cutoff);
        if (recent.length === 0) continue;
        const displayName = exData.swappedTo?.name || ex.name;
        const unit = exData.swappedTo?.unit || ex.unit;
        for (const sess of recent) {
          dayEntries.push({
            date: sess.date, name: displayName, unit, sets: sess.sets,
            note: sess.note, difficultyRating: sess.difficultyRating, prHit: sess.prHit,
            swapped: !!exData.swappedTo,
          });
        }
      }
      if (dayEntries.length === 0) continue;
      lines.push('');
      lines.push(`${getDayName(dayKey).toUpperCase()} — ${day.subtitle}`);
      const byDate = {};
      for (const s of dayEntries) {
        const k = s.date.slice(0, 10);
        if (!byDate[k]) byDate[k] = [];
        byDate[k].push(s);
      }
      const dates = Object.keys(byDate).sort().reverse();
      for (const d of dates) {
        lines.push(`  ${d}`);
        for (const s of byDate[d]) {
          const setStrs = s.sets.map(set => {
            if (set.weight === 'BW') return `BW×${set.reps}`;
            if (s.unit === 'plates') {
              const lbs = Math.round((parseFloat(set.weight) || 0) * plateLb);
              return `${set.weight}p(${lbs}lb)×${set.reps}`;
            }
            return `${set.weight}lb×${set.reps}`;
          }).join(', ');
          lines.push(`    • ${s.name}${s.swapped ? ' [swap]' : ''}${s.prHit ? ' ★PR' : ''}: ${setStrs}`);
          if (s.difficultyRating) lines.push(`        ↳ felt: ${s.difficultyRating}`);
          if (s.note) lines.push(`        ↳ note: ${s.note}`);
        }
      }
    }
    const recentMoods = (meta.dayMoods || []).filter(m => new Date(m.date).getTime() >= cutoff);
    if (recentMoods.length > 0) {
      lines.push('');
      lines.push('POST-WORKOUT MOOD');
      for (const m of recentMoods) {
        lines.push(`  ${m.date.slice(0, 10)} (${getDayName(m.day)}): ${m.mood}`);
      }
    }
    const recentCardio = (meta.cardio || []).filter(c => new Date(c.date).getTime() >= cutoff);
    if (recentCardio.length > 0) {
      lines.push('');
      lines.push('CARDIO / REST');
      for (const c of recentCardio) {
        const parts = [c.type];
        if (c.duration) parts.push(`${c.duration} min`);
        if (c.distance) parts.push(`${c.distance} km`);
        if (c.pace) parts.push(c.pace);
        if (c.effort) parts.push(`effort: ${c.effort}`);
        if (c.restReason) parts.push(`reason: ${c.restReason}`);
        if (c.note) parts.push(`"${c.note}"`);
        lines.push(`  ${c.date.slice(0, 10)}: ${parts.join(' — ')}`);
      }
    }
    if (lines.length === 7) { lines.push(''); lines.push('(no sessions in this scope yet)'); }
    return lines.join('\n');
  }
  const text = generate();
  async function doCopy() {
    try { await navigator.clipboard.writeText(text); setCopied(true); setTimeout(() => setCopied(false), 2000); }
    catch (e) {
      const ta = document.createElement('textarea');
      ta.value = text; document.body.appendChild(ta); ta.select();
      try { document.execCommand('copy'); setCopied(true); setTimeout(() => setCopied(false), 2000); } catch {}
      document.body.removeChild(ta);
    }
  }
  return (
    <div style={S.modalOverlay} onClick={onClose}>
      <div style={S.modalBox} onClick={(e) => e.stopPropagation()}>
        <div style={S.modalHeader}>
          <div><div style={{ fontSize: 10, letterSpacing: '0.2em', color: '#6e5d4d' }}>EXPORT</div>
            <div style={{ fontSize: 18, fontWeight: 700, marginTop: 4 }}>Recap for Claude</div></div>
          <button style={S.modalClose} onClick={onClose}><X size={16} /></button>
        </div>
        <div style={{ display: 'flex', gap: 6, marginBottom: 12, flexWrap: 'wrap' }}>
          {['7', '14', '30', 'all'].map(s => (
            <button key={s} onClick={() => setScope(s)}
              style={{ ...S.scopeBtn, background: scope === s ? '#e85d4a' : 'transparent', color: scope === s ? '#1a1410' : '#a89580', borderColor: scope === s ? '#e85d4a' : '#4a3a28' }}>
              {s === 'all' ? 'all time' : `${s}d`}
            </button>
          ))}
        </div>
        <pre style={S.exportPre}>{text}</pre>
        <button style={S.copyBtn} onClick={doCopy}>
          {copied ? <><Check size={14} /> copied — paste to Claude</> : <><FileDown size={14} /> copy to clipboard</>}
        </button>
      </div>
    </div>
  );
}

function DeloadModal({ onClose, onMarkDone }) {
  return (
    <div style={S.modalOverlay} onClick={onClose}>
      <div style={S.modalBox} onClick={(e) => e.stopPropagation()}>
        <div style={S.modalHeader}>
          <div><div style={{ fontSize: 10, letterSpacing: '0.2em', color: '#e85d4a' }}>DELOAD WEEK</div>
            <div style={{ fontSize: 18, fontWeight: 700, marginTop: 4 }}>Time to back off — on purpose.</div></div>
          <button style={S.modalClose} onClick={onClose}><X size={16} /></button>
        </div>
        <div style={{ color: '#f5ead9', fontSize: 13, lineHeight: 1.7 }}>
          You've trained hard for ~6 weeks. Your nervous system, joints, and tendons need a break.
          <br /><br />
          <strong style={{ color: '#f5ead9' }}>What to do this week:</strong><br /><br />
          <span style={{ color: '#a89580' }}>
            • Same exercises, same days<br />
            • Drop your weights by 40-50%<br />
            • Stop 4-5 reps shy of failure<br />
            • Should feel almost easy — that's the point<br /><br />
            Next week, come back stronger.
          </span>
        </div>
        <button style={S.copyBtn} onClick={onMarkDone}><Check size={14} /> I'll deload this week</button>
      </div>
    </div>
  );
}

function ConfirmModal({ title, body, confirmText, onConfirm, onCancel }) {
  return (
    <div style={S.modalOverlay} onClick={onCancel}>
      <div style={{ ...S.modalBox, maxWidth: 360 }} onClick={(e) => e.stopPropagation()}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 12 }}>
          <AlertCircle size={18} color="#e85d4a" />
          <div style={{ fontSize: 16, fontWeight: 700 }}>{title}</div>
        </div>
        <div style={{ color: '#a89580', fontSize: 13, lineHeight: 1.5, marginBottom: 16 }}>{body}</div>
        <div style={{ display: 'flex', gap: 8 }}>
          <button style={S.cancelBtn} onClick={onCancel}>Keep editing</button>
          <button style={{ ...S.copyBtn, marginTop: 0, background: '#e85d4a' }} onClick={onConfirm}>{confirmText}</button>
        </div>
      </div>
    </div>
  );
}

function MoodModal({ onPick, onSkip }) {
  const moods = [
    { id: 'great', label: 'Great', icon: Smile, color: '#5b9279' },
    { id: 'good', label: 'Good', icon: Smile, color: '#d4a017' },
    { id: 'off', label: 'Off', icon: Meh, color: '#999' },
    { id: 'bad', label: 'Bad', icon: Frown, color: '#e85d4a' },
  ];
  return (
    <div style={S.modalOverlay} onClick={onSkip}>
      <div style={{ ...S.modalBox, maxWidth: 360 }} onClick={(e) => e.stopPropagation()}>
        <div style={S.modalHeader}>
          <div><div style={{ fontSize: 10, letterSpacing: '0.2em', color: '#6e5d4d' }}>CHECK-IN</div>
            <div style={{ fontSize: 16, fontWeight: 700, marginTop: 4 }}>How was the session?</div></div>
          <button style={S.modalClose} onClick={onSkip}><X size={16} /></button>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
          {moods.map(m => {
            const Icon = m.icon;
            return (
              <button key={m.id} style={{ ...S.moodBtn, borderColor: m.color, color: m.color }} onClick={() => onPick(m.id)}>
                <Icon size={20} /><span>{m.label}</span>
              </button>
            );
          })}
        </div>
        <button style={{ ...S.smallLink, margin: '12px auto 0', display: 'flex' }} onClick={onSkip}>skip</button>
      </div>
    </div>
  );
}

// ─── Trophy Unlock Modal (BIG) ─────────────────────
function TrophyUnlockModal({ trophy, onClose }) {
  return (
    <div style={S.modalOverlay} onClick={onClose}>
      <div style={{ ...S.modalBox, maxWidth: 380, padding: '32px 24px 24px', textAlign: 'center' }} onClick={(e) => e.stopPropagation()}>
        <div style={{ fontSize: 10, letterSpacing: '0.4em', color: '#e85d4a', fontWeight: 700, marginBottom: 16 }}>
          TROPHY UNLOCKED
        </div>
        <div style={{ ...S.trophyIconBig, animation: 'trophyPop 0.6s cubic-bezier(0.34, 1.56, 0.64, 1)' }}>
          <TrophyIcon type={trophy.icon} size={72} color="#e85d4a" />
        </div>
        <div style={{ fontSize: 22, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif", letterSpacing: '0.04em', marginTop: 18, color: '#f5ead9', textTransform: 'uppercase' }}>
          {trophy.name}
        </div>
        <div style={{ fontSize: 13, color: '#a89580', marginTop: 8, lineHeight: 1.5 }}>{trophy.desc}</div>
        <button style={{ ...S.copyBtn, marginTop: 22 }} onClick={onClose}>nice</button>
      </div>
      <style>{`@keyframes trophyPop { 0% { transform: scale(0) rotate(-180deg); opacity: 0; } 100% { transform: scale(1) rotate(0); opacity: 1; } }`}</style>
    </div>
  );
}

// ─── CARDIO TAB ────────────────────────────────────
function CardioView({ meta, onLog, onUpdate, onDelete }) {
  const [type, setType] = useState('run');
  const [duration, setDuration] = useState('');
  const [distance, setDistance] = useState('');
  const [effort, setEffort] = useState(null);
  const [restReason, setRestReason] = useState(null);
  const [note, setNote] = useState('');
  const [saved, setSaved] = useState(false);
  const [editingIdx, setEditingIdx] = useState(null);
  const [confirmDelete, setConfirmDelete] = useState(null);

  const types = [
    { id: 'run', label: 'Run', hasDistance: true },
    { id: 'walk', label: 'Walk', hasDistance: true },
    { id: 'treadmill', label: 'Treadmill', hasDistance: true },
    { id: 'rest', label: 'Rest day', hasDistance: false },
    { id: 'other', label: 'Other', hasDistance: false },
  ];
  const currentType = types.find(t => t.id === type);
  const isRest = type === 'rest';

  const efforts = [
    { id: 'easy', label: 'Easy', color: '#5b9279' },
    { id: 'moderate', label: 'Moderate', color: '#d4a017' },
    { id: 'hard', label: 'Hard', color: '#e85d4a' },
  ];

  const restReasons = [
    { id: 'planned', label: 'Planned' },
    { id: 'sore', label: 'Sore' },
    { id: 'sick', label: 'Sick' },
    { id: 'busy', label: 'Busy' },
  ];

  const presets = [
    { label: '20m easy walk', type: 'walk', duration: 20, effort: 'easy' },
    { label: '30m easy run', type: 'run', duration: 30, effort: 'easy' },
    { label: '20m treadmill', type: 'treadmill', duration: 20, effort: 'easy' },
    { label: 'Rest — sore', type: 'rest', restReason: 'sore' },
  ];

  function applyPreset(p) {
    setType(p.type);
    setDuration(p.duration ? p.duration.toString() : '');
    setDistance('');
    setEffort(p.effort || null);
    setRestReason(p.restReason || null);
    setNote('');
  }

  function resetForm() {
    setDuration(''); setDistance(''); setEffort(null); setRestReason(null); setNote('');
    setEditingIdx(null);
  }

  function calcPace(dur, dist) {
    const d = parseFloat(dur), km = parseFloat(dist);
    if (!d || !km || km <= 0) return null;
    const paceMin = d / km;
    const m = Math.floor(paceMin);
    const s = Math.round((paceMin - m) * 60);
    return `${m}:${s.toString().padStart(2, '0')}/km`;
  }

  function submit() {
    if (!isRest && !duration) return;
    const entry = {
      date: editingIdx !== null
        ? (meta.cardio[meta.cardio.length - 1 - editingIdx]?.date || new Date().toISOString())
        : new Date().toISOString(),
      type: currentType.label,
      typeId: type,
      duration: isRest ? null : parseInt(duration),
      distance: !isRest && currentType.hasDistance && distance ? parseFloat(distance) : null,
      pace: !isRest && currentType.hasDistance && duration && distance ? calcPace(duration, distance) : null,
      effort: !isRest ? effort : null,
      restReason: isRest ? restReason : null,
      note,
    };
    if (editingIdx !== null) {
      onUpdate(meta.cardio.length - 1 - editingIdx, entry);
    } else {
      onLog(entry);
    }
    resetForm();
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  }

  function startEdit(reverseIdx) {
    const entry = (meta.cardio || []).slice(-30).reverse()[reverseIdx];
    if (!entry) return;
    setEditingIdx(reverseIdx);
    setType(entry.typeId || (entry.type === 'Rest day' ? 'rest' : entry.type === 'Run' ? 'run' : entry.type === 'Walk' ? 'walk' : entry.type === 'Treadmill' ? 'treadmill' : 'other'));
    setDuration(entry.duration ? entry.duration.toString() : '');
    setDistance(entry.distance ? entry.distance.toString() : '');
    setEffort(entry.effort || null);
    setRestReason(entry.restReason || null);
    setNote(entry.note || '');
  }

  // ─── Weekly stats ───
  const now = Date.now();
  const weekAgo = now - 7 * 86400000;
  const thisWeek = (meta.cardio || []).filter(c => new Date(c.date).getTime() >= weekAgo);
  const minutesThisWeek = thisWeek.reduce((sum, c) => sum + (c.duration || 0), 0);
  const kmThisWeek = thisWeek.reduce((sum, c) => sum + (c.distance || 0), 0);
  const restDaysThisWeek = thisWeek.filter(c => c.typeId === 'rest' || c.type === 'Rest day').length;
  const longestRun = (meta.cardio || []).reduce((max, c) => {
    if ((c.typeId === 'run' || c.type === 'Run') && c.distance && c.distance > max) return c.distance;
    return max;
  }, 0);

  const recent = (meta.cardio || []).slice(-30).reverse();
  const pace = calcPace(duration, distance);

  return (
    <div>
      <h1 style={S.h1}>Cardio & rest.</h1>
      <p style={S.h1Sub}>Watch records the run, you log the basics here.</p>

      {/* Weekly stats */}
      {(meta.cardio || []).length > 0 && (
        <div style={{ ...S.statsGrid, marginBottom: 18 }}>
          <div style={S.statCard}>
            <div style={S.statVal}>{minutesThisWeek}</div>
            <div style={S.statLabel}>MIN / WEEK</div>
          </div>
          <div style={S.statCard}>
            <div style={S.statVal}>{kmThisWeek > 0 ? kmThisWeek.toFixed(1) : '—'}</div>
            <div style={S.statLabel}>KM / WEEK</div>
          </div>
          <div style={S.statCard}>
            <div style={S.statVal}>{restDaysThisWeek}</div>
            <div style={S.statLabel}>REST DAYS</div>
          </div>
        </div>
      )}

      {/* Quick presets */}
      <div style={{ marginBottom: 14 }}>
        <div style={{ ...S.sectionLabel, marginBottom: 8 }}>QUICK LOG</div>
        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
          {presets.map((p, i) => (
            <button key={i} onClick={() => applyPreset(p)}
              style={{ ...S.scopeBtn, background: 'transparent', color: '#a89580', borderColor: '#4a3a28', fontSize: 10 }}>
              {p.label}
            </button>
          ))}
        </div>
      </div>

      {/* Log form */}
      <div style={{ background: '#241b15', border: '1px solid #1f1f1f', borderRadius: 4, padding: 16, marginBottom: 20 }}>
        {editingIdx !== null && (
          <div style={{ fontSize: 10, color: '#d4a017', letterSpacing: '0.2em', marginBottom: 10, display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <span>EDITING ENTRY</span>
            <button onClick={resetForm} style={{ ...S.smallLink, padding: 0 }}>cancel</button>
          </div>
        )}

        <div style={S.inputLabel}>TYPE</div>
        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 14 }}>
          {types.map(t => (
            <button key={t.id} onClick={() => setType(t.id)}
              style={{
                ...S.scopeBtn,
                background: type === t.id ? '#e85d4a' : 'transparent',
                color: type === t.id ? '#1a1410' : '#a89580',
                borderColor: type === t.id ? '#e85d4a' : '#4a3a28',
              }}>
              {t.label}
            </button>
          ))}
        </div>

        {!isRest && (
          <>
            <div style={{ display: 'flex', gap: 10, marginBottom: 14 }}>
              <div style={{ flex: 1 }}>
                <div style={S.inputLabel}>DURATION (MIN)</div>
                <input type="text" inputMode="numeric" value={duration} onChange={(e) => setDuration(e.target.value)}
                  placeholder="30" style={{ ...S.numInput, border: '1px solid #2a2a2a', borderRadius: 3, height: 38, textAlign: 'center' }} />
              </div>
              {currentType.hasDistance && (
                <div style={{ flex: 1 }}>
                  <div style={S.inputLabel}>DISTANCE (KM)</div>
                  <input type="text" inputMode="decimal" value={distance} onChange={(e) => setDistance(e.target.value)}
                    placeholder="5.0" style={{ ...S.numInput, border: '1px solid #2a2a2a', borderRadius: 3, height: 38, textAlign: 'center' }} />
                </div>
              )}
            </div>

            {pace && (
              <div style={{ fontSize: 11, color: '#a89580', marginBottom: 14, textAlign: 'center', letterSpacing: '0.05em' }}>
                avg pace · <span style={{ color: '#e85d4a', fontWeight: 600 }}>{pace}</span>
              </div>
            )}

            <div style={S.inputLabel}>EFFORT</div>
            <div style={{ display: 'flex', gap: 6, marginBottom: 14 }}>
              {efforts.map(e => (
                <button key={e.id} onClick={() => setEffort(effort === e.id ? null : e.id)}
                  style={{
                    flex: 1,
                    background: effort === e.id ? e.color : 'transparent',
                    color: effort === e.id ? '#1a1410' : '#a89580',
                    border: `1px solid ${effort === e.id ? e.color : '#4a3a28'}`,
                    padding: '7px 8px', borderRadius: 3, fontFamily: 'inherit', fontSize: 11, cursor: 'pointer', letterSpacing: '0.05em',
                  }}>
                  {e.label}
                </button>
              ))}
            </div>
          </>
        )}

        {isRest && (
          <>
            <div style={S.inputLabel}>REASON</div>
            <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 14 }}>
              {restReasons.map(r => (
                <button key={r.id} onClick={() => setRestReason(restReason === r.id ? null : r.id)}
                  style={{
                    ...S.scopeBtn,
                    background: restReason === r.id ? '#5b9279' : 'transparent',
                    color: restReason === r.id ? '#1a1410' : '#a89580',
                    borderColor: restReason === r.id ? '#5b9279' : '#4a3a28',
                  }}>
                  {r.label}
                </button>
              ))}
            </div>
          </>
        )}

        <div style={S.inputLabel}>NOTE (OPTIONAL)</div>
        <input type="text" value={note} onChange={(e) => setNote(e.target.value)}
          placeholder={isRest ? 'lower back tight, taking the day' : 'felt easy, kept HR zone 2'}
          style={{ ...S.numInput, border: '1px solid #2a2a2a', borderRadius: 3, height: 38, textAlign: 'left', padding: '0 12px', marginBottom: 14, fontWeight: 400 }} />

        <button style={{ ...S.copyBtn, marginTop: 0, background: (isRest || duration) ? '#e85d4a' : '#2e2419', color: (isRest || duration) ? '#1a1410' : '#6e5d4d' }}
          onClick={submit} disabled={!isRest && !duration}>
          {saved ? <><Check size={14} /> {editingIdx !== null ? 'updated' : 'logged'}</> : (editingIdx !== null ? 'save changes' : 'log it')}
        </button>
      </div>

      {/* Recent + edit/delete */}
      <div style={S.sectionLabel}>RECENT</div>
      {recent.length === 0 ? (
        <div style={{ color: '#6e5d4d', fontSize: 12, padding: 16, textAlign: 'center', border: '1px dashed #2a2a2a', borderRadius: 4 }}>
          Nothing logged yet.
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {recent.map((c, i) => {
            const effortMeta = efforts.find(e => e.id === c.effort);
            return (
              <div key={i} style={{ background: '#241b15', border: '1px solid #1f1f1f', borderRadius: 3, padding: '10px 12px', display: 'flex', alignItems: 'center', gap: 10 }}>
                <Activity size={14} color="#666" />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13, color: '#f5ead9', fontWeight: 600, display: 'flex', alignItems: 'baseline', gap: 6, flexWrap: 'wrap' }}>
                    <span>{c.type}</span>
                    {c.duration && <span style={{ color: '#a89580', fontWeight: 400, fontSize: 12 }}>{c.duration}m</span>}
                    {c.distance && <span style={{ color: '#a89580', fontWeight: 400, fontSize: 12 }}>· {c.distance}km</span>}
                    {c.pace && <span style={{ color: '#6e5d4d', fontWeight: 400, fontSize: 11 }}>· {c.pace}</span>}
                    {effortMeta && <span style={{ fontSize: 9, color: effortMeta.color, letterSpacing: '0.1em', border: `1px solid ${effortMeta.color}`, padding: '1px 4px', borderRadius: 2 }}>{effortMeta.label.toUpperCase()}</span>}
                    {c.restReason && <span style={{ fontSize: 9, color: '#a89580', letterSpacing: '0.1em' }}>· {c.restReason}</span>}
                  </div>
                  {c.note && <div style={{ fontSize: 11, color: '#a89580', marginTop: 2, fontStyle: 'italic' }}>"{c.note}"</div>}
                </div>
                <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 4 }}>
                  <div style={{ fontSize: 10, color: '#6e5d4d' }}>{fmtDateShort(c.date)}</div>
                  <div style={{ display: 'flex', gap: 4 }}>
                    <button onClick={() => startEdit(i)} style={{ ...S.iconBtnSmall, width: 22, height: 22 }}><Edit2 size={10} /></button>
                    <button onClick={() => setConfirmDelete(i)} style={{ ...S.iconBtnSmall, width: 22, height: 22 }}><X size={11} /></button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {confirmDelete !== null && (
        <ConfirmModal
          title="Delete entry?"
          body="This cardio entry will be removed permanently."
          confirmText="Delete"
          onConfirm={() => { onDelete(meta.cardio.length - 1 - confirmDelete); setConfirmDelete(null); }}
          onCancel={() => setConfirmDelete(null)}
        />
      )}

      {/* Stats: longest run */}
      {longestRun > 0 && (
        <div style={{ marginTop: 18, background: '#241b15', border: '1px solid #1f1f1f', borderRadius: 3, padding: '12px 14px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <span style={{ fontSize: 11, color: '#a89580', letterSpacing: '0.1em' }}>LONGEST RUN</span>
          <span style={{ fontSize: 14, color: '#e85d4a', fontWeight: 700 }}>{longestRun.toFixed(2)} km</span>
        </div>
      )}

      <div style={S.tipBlock}>
        <div style={S.tipLabel}>Galaxy Watch sync</div>
        <div style={S.tipText}>Your watch records the run automatically. Open Samsung Health, read the distance + time + pace, type 'em here. Takes 10 seconds. (When this becomes a real app, we'll wire up Strava to auto-pull.)</div>
      </div>

      <div style={{ ...S.tipBlock, marginTop: 14 }}>
        <div style={S.tipLabel}>Pacing rule</div>
        <div style={S.tipText}>Hard cardio the day before legs = bad legs day. Keep runs easy on lifting-adjacent days, hard ones only when you've got recovery time.</div>
      </div>
    </div>
  );
}

// ─── ANALYTICS — TIER 1 FULL ──────────────────────
function AnalyticsView({ data, meta, program, getDayName, plateLb }) {
  const totalSessions = meta.totalSessions || 0;
  const [selectedExId, setSelectedExId] = useState(null);

  if (totalSessions === 0) {
    return (
      <div>
        <h1 style={S.h1}>Stats.</h1>
        <p style={S.h1Sub}>This is where your progress shows up.</p>
        <div style={S.emptyState}>
          <BarChart3 size={36} color="#444" style={{ marginBottom: 16 }} />
          <div style={{ fontSize: 14, color: '#a89580', marginBottom: 8, fontWeight: 600 }}>No data yet.</div>
          <div style={{ fontSize: 12, color: '#6e5d4d', lineHeight: 1.6, textAlign: 'center', maxWidth: 280 }}>
            Log your first session to see this come to life. Stats appear as soon as you finish one exercise.
          </div>
        </div>
        <div style={S.tipBlock}>
          <div style={S.tipLabel}>What you'll see here</div>
          <div style={S.tipText}>Strength curves per exercise · Weekly volume per muscle group · Frequency heatmap · Personal records timeline</div>
        </div>
      </div>
    );
  }

  // Build PR list
  const prList = [];
  for (const [exId, exData] of Object.entries(data)) {
    if (!exData.sessions) continue;
    for (const sess of exData.sessions) {
      if (sess.prHit) {
        const ex = findExercise(exId, program);
        if (!ex) continue;
        const displayName = exData.swappedTo?.name || ex.name;
        const unit = exData.swappedTo?.unit || ex.unit;
        const bestSet = sess.sets.reduce((best, s) => {
          const w = s.weight === 'BW' ? 0 : parseFloat(s.weight) || 0;
          const r = parseInt(s.reps) || 0;
          const bw = best ? (best.weight === 'BW' ? 0 : parseFloat(best.weight) || 0) : 0;
          const br = best ? parseInt(best.reps) || 0 : 0;
          return (w > bw || (w === bw && r > br)) ? s : best;
        }, null);
        if (bestSet) prList.push({ date: sess.date, name: displayName, unit, set: bestSet });
      }
    }
  }
  prList.sort((a, b) => new Date(b.date) - new Date(a.date));

  // Weekly volume per muscle (last 4 weeks)
  const weekVolume = {}; // { weekStart: { muscle: volume } }
  const now = new Date();
  const cutoff = now.getTime() - 28 * 86400000;
  for (const [exId, exData] of Object.entries(data)) {
    if (!exData.sessions) continue;
    const ex = findExercise(exId, program);
    if (!ex) continue;
    for (const sess of exData.sessions) {
      if (new Date(sess.date).getTime() < cutoff) continue;
      const d = new Date(sess.date);
      const weekStart = new Date(d);
      weekStart.setDate(d.getDate() - d.getDay());
      weekStart.setHours(0, 0, 0, 0);
      const key = weekStart.toISOString().slice(0, 10);
      if (!weekVolume[key]) weekVolume[key] = {};
      let vol = 0;
      for (const s of sess.sets) {
        const w = s.weight === 'BW' ? 0 : parseFloat(s.weight) || 0;
        const r = parseInt(s.reps) || 0;
        vol += (ex.unit === 'plates' ? w * plateLb : w) * r;
      }
      weekVolume[key][ex.muscle] = (weekVolume[key][ex.muscle] || 0) + vol;
    }
  }

  // Frequency heatmap — last 49 days (7 weeks)
  const heatmap = [];
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  for (let i = 48; i >= 0; i--) {
    const d = new Date(today.getTime() - i * 86400000);
    const dStr = d.toISOString().slice(0, 10);
    let count = 0;
    for (const exData of Object.values(data)) {
      if (!exData.sessions) continue;
      for (const sess of exData.sessions) {
        if (sess.date.slice(0, 10) === dStr) count++;
      }
    }
    heatmap.push({ date: dStr, count, dayOfWeek: d.getDay() });
  }

  // Strength curves — list exercises with 2+ sessions
  const strengthExercises = [];
  for (const [exId, exData] of Object.entries(data)) {
    if (!exData.sessions || exData.sessions.length < 2) continue;
    const ex = findExercise(exId, program);
    if (!ex) continue;
    strengthExercises.push({ id: exId, name: exData.swappedTo?.name || ex.name, unit: exData.swappedTo?.unit || ex.unit, sessions: exData.sessions });
  }

  return (
    <div>
      <h1 style={S.h1}>Stats.</h1>
      <p style={S.h1Sub}>Tier 1 metrics</p>

      {/* Summary */}
      <div style={S.statsGrid}>
        <div style={S.statCard}>
          <div style={S.statVal}>{totalSessions}</div>
          <div style={S.statLabel}>EXERCISES</div>
        </div>
        <div style={S.statCard}>
          <div style={S.statVal}>{meta.workoutsCompleted || 0}</div>
          <div style={S.statLabel}>WORKOUTS</div>
        </div>
        <div style={S.statCard}>
          <div style={S.statVal}>{prList.length}</div>
          <div style={S.statLabel}>PRs</div>
        </div>
      </div>

      {/* Heatmap */}
      <div style={{ marginTop: 28 }}>
        <div style={S.sectionLabel}>FREQUENCY (LAST 7 WEEKS)</div>
        <FrequencyHeatmap data={heatmap} />
      </div>

      {/* Strength curve picker */}
      <div style={{ marginTop: 28 }}>
        <div style={S.sectionLabel}>STRENGTH CURVE</div>
        {strengthExercises.length === 0 ? (
          <div style={{ color: '#6e5d4d', fontSize: 12, padding: 16, textAlign: 'center', border: '1px dashed #2a2a2a', borderRadius: 4 }}>
            Log an exercise at least twice to see its strength curve.
          </div>
        ) : (
          <>
            <select value={selectedExId || strengthExercises[0].id} onChange={(e) => setSelectedExId(e.target.value)}
              style={{ width: '100%', background: '#241b15', border: '1px solid #2a2a2a', color: '#f5ead9', padding: '10px 12px', borderRadius: 3, fontFamily: 'inherit', fontSize: 12, marginBottom: 12 }}>
              {strengthExercises.map(ex => (<option key={ex.id} value={ex.id}>{ex.name}</option>))}
            </select>
            <StrengthCurve exercise={strengthExercises.find(e => e.id === (selectedExId || strengthExercises[0].id))} plateLb={plateLb} />
          </>
        )}
      </div>

      {/* Weekly volume */}
      <div style={{ marginTop: 28 }}>
        <div style={S.sectionLabel}>WEEKLY VOLUME BY MUSCLE</div>
        <WeeklyVolumeChart weekVolume={weekVolume} />
      </div>

      {/* PR timeline */}
      <div style={{ marginTop: 28 }}>
        <div style={S.sectionLabel}>PERSONAL RECORDS</div>
        {prList.length === 0 ? (
          <div style={{ color: '#6e5d4d', fontSize: 12, padding: 16, textAlign: 'center', border: '1px dashed #2a2a2a', borderRadius: 4 }}>
            No PRs yet.
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {prList.slice(0, 30).map((pr, i) => (
              <div key={i} style={{ background: '#241b15', border: '1px solid #1f1f1f', borderRadius: 3, padding: '10px 12px', display: 'flex', alignItems: 'center', gap: 10 }}>
                <Trophy size={12} color="#e85d4a" />
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: 12, color: '#f5ead9', fontWeight: 600 }}>{pr.name}</div>
                  <div style={{ fontSize: 11, color: '#a89580', marginTop: 2 }}>
                    {pr.set.weight === 'BW' ? 'BW' : `${pr.set.weight}${pr.unit === 'plates' ? 'p' : 'lb'}`} × {pr.set.reps}
                  </div>
                </div>
                <div style={{ fontSize: 10, color: '#6e5d4d' }}>{fmtDateShort(pr.date)}</div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function findExercise(exId, program) {
  for (const day of Object.values(program)) {
    for (const ex of day.exercises) if (ex.id === exId) return ex;
  }
  return null;
}

function FrequencyHeatmap({ data }) {
  // 7 rows (days of week) × 7 cols (weeks)
  // data is 49 days, oldest first, grouped by day-of-week
  const cellSize = 18, gap = 3;
  const cols = 7;
  return (
    <div style={{ display: 'flex', justifyContent: 'center', padding: '8px 0' }}>
      <svg width={cols * (cellSize + gap)} height={7 * (cellSize + gap) + 14}>
        {['M', 'T', 'W', 'T', 'F', 'S', 'S'].map((l, i) => (
          <text key={i} x={-2} y={(i + 1) * (cellSize + gap) - 4} fontSize="8" fill="#444" textAnchor="end" fontFamily="monospace">{l}</text>
        ))}
        {data.map((d, i) => {
          const col = Math.floor(i / 7);
          const row = (d.dayOfWeek + 6) % 7; // Mon = 0
          let bg = '#1f1812';
          if (d.count >= 1) bg = '#3a2418';
          if (d.count >= 3) bg = '#7a3a22';
          if (d.count >= 5) bg = '#c45a3a';
          if (d.count >= 7) bg = '#e85d4a';
          return (
            <rect key={i} x={col * (cellSize + gap) + 14} y={row * (cellSize + gap)}
              width={cellSize} height={cellSize} fill={bg} rx={2} />
          );
        })}
      </svg>
    </div>
  );
}

function StrengthCurve({ exercise, plateLb }) {
  if (!exercise) return null;
  const points = exercise.sessions.map(s => {
    const top = s.sets.reduce((max, set) => {
      const w = set.weight === 'BW' ? 0 : parseFloat(set.weight) || 0;
      return w > max ? w : max;
    }, 0);
    return { date: new Date(s.date), value: exercise.unit === 'plates' ? top * plateLb : top };
  });
  if (points.length < 2) return null;
  const max = Math.max(...points.map(p => p.value), 1);
  const min = Math.min(...points.map(p => p.value), 0);
  const range = max - min || 1;
  const w = 280, h = 100;
  const step = w / (points.length - 1);
  const linePts = points.map((p, i) => `${i * step},${h - ((p.value - min) / range) * (h - 10) - 5}`).join(' ');
  return (
    <div style={{ background: '#241b15', border: '1px solid #1f1f1f', borderRadius: 3, padding: '14px 16px' }}>
      <svg width="100%" height={h} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none">
        <polyline points={linePts} fill="none" stroke="#e85d4a" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
        {points.map((p, i) => (
          <circle key={i} cx={i * step} cy={h - ((p.value - min) / range) * (h - 10) - 5} r="3" fill="#e85d4a" />
        ))}
      </svg>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 8, fontSize: 10, color: '#6e5d4d' }}>
        <span>{points[0].date.toISOString().slice(5, 10)}</span>
        <span>↑ {max}{exercise.unit === 'plates' ? 'lb (incl. plate conv)' : 'lb'}</span>
        <span>{points[points.length - 1].date.toISOString().slice(5, 10)}</span>
      </div>
    </div>
  );
}

function WeeklyVolumeChart({ weekVolume }) {
  const weeks = Object.keys(weekVolume).sort();
  if (weeks.length === 0) {
    return <div style={{ color: '#6e5d4d', fontSize: 12, padding: 16, textAlign: 'center', border: '1px dashed #2a2a2a', borderRadius: 4 }}>Need at least one week of data.</div>;
  }
  const muscleSet = new Set();
  for (const w of weeks) {
    for (const m of Object.keys(weekVolume[w])) muscleSet.add(m);
  }
  const muscles = Array.from(muscleSet);
  const allVals = weeks.flatMap(w => muscles.map(m => weekVolume[w][m] || 0));
  const maxVal = Math.max(...allVals, 1);
  return (
    <div style={{ background: '#241b15', border: '1px solid #1f1f1f', borderRadius: 3, padding: '14px 16px' }}>
      {muscles.map(muscle => (
        <div key={muscle} style={{ marginBottom: 10 }}>
          <div style={{ fontSize: 10, color: '#a89580', textTransform: 'capitalize', marginBottom: 3 }}>{muscle}</div>
          <div style={{ display: 'flex', gap: 4, alignItems: 'flex-end', height: 30 }}>
            {weeks.map(w => {
              const v = weekVolume[w][muscle] || 0;
              const pct = (v / maxVal) * 100;
              return (
                <div key={w} style={{ flex: 1, background: '#2e2419', borderRadius: 2, height: '100%', display: 'flex', alignItems: 'flex-end' }}>
                  <div style={{ width: '100%', height: `${pct}%`, background: '#e85d4a', borderRadius: 2 }} />
                </div>
              );
            })}
          </div>
        </div>
      ))}
      <div style={{ fontSize: 9, color: '#6e5d4d', marginTop: 8, letterSpacing: '0.1em', textAlign: 'center' }}>
        OLDEST → NEWEST · MAX {Math.round(maxVal).toLocaleString()} LB
      </div>
    </div>
  );
}

// ─── TROPHIES TAB ──────────────────────────────────
function TrophiesView({ unlocked, stats }) {
  const byCategory = {
    firsts: { label: 'First-Time Hits', items: [] },
    consistency: { label: 'Consistency', items: [] },
    strength: { label: 'Strength Milestones', items: [] },
  };
  for (const t of TROPHIES) byCategory[t.category].items.push(t);

  return (
    <div>
      <h1 style={S.h1}>Trophy room.</h1>
      <p style={S.h1Sub}>{unlocked.length} / {TROPHIES.length} unlocked</p>

      {Object.entries(byCategory).map(([key, cat]) => (
        <div key={key} style={{ marginTop: 24 }}>
          <div style={S.sectionLabel}>{cat.label.toUpperCase()}</div>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
            {cat.items.map(t => {
              const isUnlocked = unlocked.includes(t.id);
              return (
                <div key={t.id} style={{
                  background: '#241b15',
                  border: `1px solid ${isUnlocked ? '#e85d4a40' : '#3a2d1f'}`,
                  borderRadius: 3, padding: '14px 12px',
                  display: 'flex', flexDirection: 'column', alignItems: 'center',
                  textAlign: 'center', opacity: isUnlocked ? 1 : 0.4,
                }}>
                  <TrophyIcon type={t.icon} size={32} color={isUnlocked ? '#e85d4a' : '#6e5d4d'} />
                  <div style={{ fontSize: 11, fontWeight: 700, color: isUnlocked ? '#f5ead9' : '#6e5d4d', marginTop: 8, letterSpacing: '0.02em' }}>
                    {t.name}
                  </div>
                  <div style={{ fontSize: 9, color: '#6e5d4d', marginTop: 4, lineHeight: 1.4 }}>{t.desc}</div>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

const globalCSS = `
  @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Bebas+Neue&display=swap');

  @keyframes fadeInUp {
    0% { opacity: 0; transform: translateY(10px); }
    100% { opacity: 1; transform: translateY(0); }
  }
  @keyframes softPulse {
    0%, 100% { box-shadow: 0 0 0 0 rgba(232, 93, 74, 0.3); }
    50% { box-shadow: 0 0 0 8px rgba(232, 93, 74, 0); }
  }
  .forge-tile:active { transform: scale(0.97); transition: transform 0.1s ease-out; }
  .forge-tile { transition: transform 0.2s ease-out, border-color 0.2s ease-out; }
  button { -webkit-tap-highlight-color: transparent; font-family: inherit; }
  body { background: #1a1410; }
  input::placeholder, textarea::placeholder { color: #6e5d4d; }
`;

// ─── Warm-brown palette ─────────────────────────────
// Background tones (darkest → lightest)
const C = {
  bg:       '#1a1410',   // page background — dark warm brown
  card:     '#241b15',   // card surface
  cardHi:   '#2e2419',   // elevated card
  cardMax:  '#3a2d1f',   // hover/active elevated
  border:   '#3a2d1f',
  borderHi: '#4a3a28',

  text:     '#f5ead9',   // primary cream text
  textDim:  '#a89580',   // secondary tan
  textMute: '#6e5d4d',   // tertiary muted

  hot:      '#e85d4a',   // primary accent — fiery orange (PRs, active, CTA)
  hotDim:   'rgba(232, 93, 74, 0.15)',
  gold:     '#d4a574',   // warm gold (trophies, premium)
  sage:     '#7ba87d',   // calm green (cardio, rest, completion)
  warn:     '#d4a017',   // warning amber (deload, WIP)
};

const S = {
  app: { fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif", background: C.bg, color: C.text, minHeight: '100vh', maxWidth: 560, margin: '0 auto', padding: '22px 18px 80px', boxSizing: 'border-box' },
  fadeIn: { animation: 'fadeInUp 0.35s ease-out' },

  // ─── Overview screen ───
  overviewDate: { fontSize: 11, color: C.textMute, letterSpacing: '0.2em', fontWeight: 600, marginBottom: 10 },
  overviewGreeting: { fontSize: 40, fontWeight: 700, margin: 0, fontFamily: "'Bebas Neue', sans-serif", letterSpacing: '0.01em', lineHeight: 1, color: C.text },

  weekStrip: { background: `linear-gradient(135deg, ${C.cardHi} 0%, ${C.card} 100%)`, border: `1px solid ${C.border}`, borderRadius: 18, padding: '18px 20px', marginBottom: 16, boxShadow: '0 4px 16px rgba(0,0,0,0.25)' },
  weekStripLabel: { fontSize: 10, color: C.textMute, letterSpacing: '0.25em', fontWeight: 600, marginBottom: 14 },
  weekStripRow: { display: 'flex', alignItems: 'center', gap: 0 },
  weekStat: { flex: 1, textAlign: 'center' },
  weekStatVal: { fontSize: 34, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif", color: C.text, lineHeight: 1 },
  weekStatLabel: { fontSize: 9, color: C.textMute, letterSpacing: '0.2em', marginTop: 6, fontWeight: 600 },
  weekStripDivider: { width: 1, height: 28, background: C.border },
  lastSession: { fontSize: 12, color: C.textDim, marginTop: 14, paddingTop: 12, borderTop: `1px solid ${C.border}`, letterSpacing: '0.01em' },

  tilePressable: { cursor: 'pointer', fontFamily: 'inherit', color: 'inherit', WebkitTapHighlightColor: 'transparent', border: 'none' },
  tileBig: { width: '100%', background: C.card, border: `1px solid ${C.border}`, borderRadius: 20, padding: 0, marginBottom: 12, position: 'relative', overflow: 'hidden', minHeight: 130, textAlign: 'left', transition: 'transform 0.15s ease-out, border-color 0.2s ease-out', boxShadow: '0 6px 20px rgba(0,0,0,0.35)' },
  tileBgGradient: { position: 'absolute', inset: 0, background: 'radial-gradient(circle at 100% 0%, rgba(232, 93, 74, 0.18) 0%, transparent 65%)', pointerEvents: 'none' },
  tileBigInner: { position: 'relative', padding: '26px 26px 24px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16, height: '100%' },
  tileLabelLg: { fontSize: 42, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif", letterSpacing: '0.03em', color: C.text, lineHeight: 1 },
  tileSubLg: { fontSize: 12, color: C.textDim, marginTop: 10, letterSpacing: '0.02em' },
  tileArrowBig: { fontSize: 32, color: C.hot, fontWeight: 300, flexShrink: 0 },

  tileRow: { display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 },
  tileSmall: { background: C.card, border: `1px solid ${C.border}`, borderRadius: 18, padding: 0, position: 'relative', overflow: 'hidden', minHeight: 108, textAlign: 'left', transition: 'transform 0.15s ease-out, border-color 0.2s ease-out', boxShadow: '0 3px 12px rgba(0,0,0,0.25)' },
  tileSmallInner: { padding: '20px 20px 18px', display: 'flex', flexDirection: 'column', justifyContent: 'flex-start', height: '100%', boxSizing: 'border-box' },
  tileLabelSm: { fontSize: 18, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif", letterSpacing: '0.03em', color: C.text, lineHeight: 1 },
  tileSubSm: { fontSize: 11, color: C.textDim, marginTop: 8, letterSpacing: '0.01em' },
  tileDisabled: { opacity: 0.5, cursor: 'not-allowed' },
  wipBadge: { fontSize: 8, letterSpacing: '0.2em', color: C.warn, border: `1px solid ${C.warn}`, padding: '2px 5px', borderRadius: 8, fontWeight: 700 },

  // ─── Section pages ───
  sectionHeader: { marginBottom: 18 },
  sectionTitle: { fontSize: 36, fontWeight: 700, margin: 0, fontFamily: "'Bebas Neue', sans-serif", letterSpacing: '0.03em', color: C.text, lineHeight: 1 },
  sectionSub: { fontSize: 12, color: C.textDim, marginTop: 8, letterSpacing: '0.02em' },
  sectionLead: { fontSize: 14, color: C.textDim, margin: '0 0 18px' },
  subtabRow: { display: 'flex', gap: 8, marginBottom: 22, padding: 4, background: C.card, borderRadius: 12, border: `1px solid ${C.border}` },
  subtab: { background: 'transparent', border: 'none', padding: '8px 18px', fontSize: 13, fontFamily: 'inherit', cursor: 'pointer', letterSpacing: '0.04em', fontWeight: 600, borderRadius: 8, flex: 1, transition: 'all 0.2s ease' },

  // ─── Brand bar ───
  brandBar: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 22, minHeight: 32 },
  brandMark: { fontSize: 12, letterSpacing: '0.35em', color: C.hot, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif" },
  exportBtn: { background: C.card, border: `1px solid ${C.border}`, color: C.textDim, padding: '8px 14px', borderRadius: 10, fontSize: 11, fontFamily: 'inherit', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 6, letterSpacing: '0.03em', fontWeight: 500 },
  backLink: { background: 'transparent', border: 'none', color: C.textDim, fontSize: 13, fontFamily: 'inherit', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 4, padding: 0, letterSpacing: '0.02em', fontWeight: 500 },
  restMini: { border: '1px solid', padding: '7px 14px', borderRadius: 10, fontSize: 11, fontFamily: 'inherit', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 6, letterSpacing: '0.03em', minWidth: 90, justifyContent: 'center', fontWeight: 600 },
  deloadBanner: { background: 'linear-gradient(135deg, #2a1a14, #241b15)', border: `1px solid ${C.hot}`, borderRadius: 14, padding: '14px 16px', marginBottom: 18, display: 'flex', alignItems: 'center', gap: 12, cursor: 'pointer', color: C.hot, boxShadow: '0 4px 16px rgba(232, 93, 74, 0.15)' },

  h1: { fontSize: 32, fontWeight: 700, margin: 0, letterSpacing: '0.01em', fontFamily: "'Bebas Neue', sans-serif", lineHeight: 1, color: C.text },
  h1Sub: { fontSize: 13, color: C.textDim, margin: '8px 0 24px', letterSpacing: '0.01em' },

  // ─── Day card ───
  dayList: { display: 'flex', flexDirection: 'column', gap: 10, marginBottom: 24 },
  dayCard: { background: C.card, border: `1px solid ${C.border}`, borderLeft: '4px solid', borderRadius: 14, padding: '16px 18px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontFamily: 'inherit', color: 'inherit', boxShadow: '0 3px 10px rgba(0,0,0,0.2)' },
  dayNameBtn: { background: 'transparent', border: 'none', padding: 0, margin: 0, cursor: 'pointer', display: 'flex', alignItems: 'center', color: 'inherit', fontFamily: 'inherit', marginBottom: 6 },
  dayName: { fontSize: 18, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif", letterSpacing: '0.03em', color: C.text },
  dayLast: { fontSize: 11, letterSpacing: '0.02em' },
  daySub: { fontSize: 12, color: C.textDim, letterSpacing: '0.01em' },
  openBtn: { background: 'transparent', border: 'none', cursor: 'pointer', padding: '8px 12px', fontFamily: 'inherit' },
  inlineEdit: { flex: 1, background: C.bg, border: `1px solid ${C.hot}`, color: C.text, padding: '8px 12px', borderRadius: 10, fontFamily: 'inherit', fontSize: 15, fontWeight: 600, outline: 'none' },
  iconBtnSmall: { background: C.bg, border: `1px solid ${C.border}`, color: C.textDim, width: 30, height: 30, borderRadius: 8, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'inherit', flexShrink: 0 },

  // ─── Warmup gate ───
  warmupGateCard: { background: C.card, border: `1px solid ${C.border}`, borderRadius: 18, padding: 22, marginTop: 8, boxShadow: '0 4px 16px rgba(0,0,0,0.25)' },
  warmupCheckRow: { display: 'flex', alignItems: 'center', gap: 14, padding: '14px 16px', background: C.bg, border: '1.5px solid', borderRadius: 12, cursor: 'pointer', fontFamily: 'inherit', textAlign: 'left', width: '100%', transition: 'all 0.2s ease' },
  warmupCheckBox: { width: 24, height: 24, border: '1.5px solid', borderRadius: 7, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 },
  warmupContinueBtn: { width: '100%', border: 'none', padding: '15px', borderRadius: 12, fontFamily: 'inherit', fontSize: 14, fontWeight: 700, letterSpacing: '0.06em', textTransform: 'uppercase', marginTop: 22, transition: 'all 0.2s ease' },
  warmupSkipBtn: { width: '100%', background: 'transparent', border: 'none', color: C.textMute, padding: '12px', fontFamily: 'inherit', fontSize: 11, cursor: 'pointer', marginTop: 8, letterSpacing: '0.05em' },

  // ─── Day workout view ───
  dayHeader: { paddingLeft: 16, marginBottom: 22 },
  dayTitle: { fontSize: 28, fontWeight: 700, margin: 0, fontFamily: "'Bebas Neue', sans-serif", letterSpacing: '0.02em', lineHeight: 1.1, color: C.text },

  // ─── Exercise card ───
  exList: { display: 'flex', flexDirection: 'column', gap: 10 },
  exCard: { background: C.card, border: `1px solid ${C.border}`, borderRadius: 14, overflow: 'hidden', boxShadow: '0 3px 10px rgba(0,0,0,0.2)' },
  exHeader: { width: '100%', background: 'transparent', border: 'none', padding: '16px 18px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', cursor: 'pointer', fontFamily: 'inherit', color: 'inherit' },
  exIdx: { fontSize: 10, color: C.textMute, fontWeight: 600, letterSpacing: '0.1em' },
  exName: { fontSize: 15, fontWeight: 600, color: C.text },
  diffTag: { fontSize: 9, letterSpacing: '0.12em', fontWeight: 600, border: '1px solid', padding: '2px 6px', borderRadius: 6 },
  swapTag: { fontSize: 9, letterSpacing: '0.12em', fontWeight: 600, textTransform: 'uppercase' },
  exMeta: { fontSize: 12, color: C.textDim, display: 'flex', alignItems: 'center', gap: 6, flexWrap: 'wrap' },
  dot: { color: C.borderHi },
  unitTag: { color: C.textMute, fontSize: 10, letterSpacing: '0.04em', textTransform: 'uppercase' },
  infoIconBtn: { display: 'inline-flex', alignItems: 'center', justifyContent: 'center', width: 30, height: 30, color: C.textDim },
  exBody: { padding: '0 18px 18px', borderTop: `1px solid ${C.border}` },
  exNote: { fontSize: 12, color: C.textDim, fontStyle: 'italic', margin: '14px 0 16px', lineHeight: 1.5 },
  lastRef: { background: C.bg, border: `1px solid ${C.border}`, borderRadius: 10, padding: '12px 14px', marginBottom: 16 },
  lastRefLabel: { color: C.textMute, letterSpacing: '0.12em', fontSize: 9, fontWeight: 600 },
  loggedSets: { display: 'flex', flexDirection: 'column', gap: 7, marginBottom: 14 },
  loggedSet: { display: 'flex', alignItems: 'center', gap: 10, padding: '10px 12px', background: C.bg, border: `1px solid ${C.border}`, borderRadius: 10 },
  removeSet: { background: 'transparent', border: 'none', color: C.textMute, fontSize: 18, cursor: 'pointer', padding: '0 6px', fontFamily: 'inherit', lineHeight: 1 },
  setHeader: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 },
  setInputRow: { display: 'flex', gap: 10, marginBottom: 12 },
  inputCol: { flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 },
  inputLabel: { fontSize: 10, color: C.textMute, letterSpacing: '0.12em', marginBottom: 7, fontWeight: 600 },
  inputWithBumps: { display: 'flex', alignItems: 'center', background: C.bg, border: `1px solid ${C.border}`, borderRadius: 10, overflow: 'hidden' },
  bump: { background: 'transparent', border: 'none', color: C.textDim, width: 36, height: 42, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'inherit', flexShrink: 0, transition: 'background 0.15s ease' },
  numInput: { flex: 1, background: 'transparent', border: 'none', color: C.text, height: 42, padding: 0, fontFamily: 'inherit', fontSize: 16, fontWeight: 600, outline: 'none', textAlign: 'center', minWidth: 0, width: '100%', boxSizing: 'border-box' },
  plateConvert: { fontSize: 10, color: C.textMute, marginTop: 5, letterSpacing: '0.03em' },
  saveSetBtn: { border: 'none', padding: '13px', width: '100%', borderRadius: 12, fontFamily: 'inherit', fontSize: 13, fontWeight: 700, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 12, transition: 'all 0.15s', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 },
  allDoneNote: { background: 'transparent', border: '1px dashed', padding: '12px', textAlign: 'center', borderRadius: 10, fontSize: 11, letterSpacing: '0.04em', marginBottom: 12 },
  ratingBlock: { background: C.bg, border: `1px solid ${C.border}`, borderRadius: 10, padding: '12px 14px', marginBottom: 12 },
  ratingLabel: { fontSize: 10, color: C.textMute, letterSpacing: '0.12em', fontWeight: 600 },
  ratingRow: { display: 'flex', gap: 6, flexWrap: 'wrap' },
  ratingBtn: { flex: '1 1 0', minWidth: 70, padding: '8px 8px', border: '1px solid', borderRadius: 8, background: 'transparent', fontFamily: 'inherit', fontSize: 11, cursor: 'pointer', letterSpacing: '0.02em', fontWeight: 500 },
  tooltipBox: { background: C.cardHi, border: `1px solid ${C.border}`, borderRadius: 8, padding: '10px 12px', marginTop: 8, fontSize: 11, color: C.textDim, lineHeight: 1.5 },
  smallLink: { background: 'transparent', border: 'none', color: C.textDim, fontSize: 12, fontFamily: 'inherit', cursor: 'pointer', padding: '6px 0', display: 'flex', alignItems: 'center', gap: 5, letterSpacing: '0.02em', fontWeight: 500 },
  noteInput: { width: '100%', background: C.bg, border: `1px solid ${C.border}`, color: C.text, padding: '10px 12px', borderRadius: 10, fontFamily: 'inherit', fontSize: 13, outline: 'none', resize: 'vertical', boxSizing: 'border-box' },
  actionRow: { display: 'flex', alignItems: 'center', gap: 12, marginTop: 12, flexWrap: 'wrap' },
  finishBtn: { marginLeft: 'auto', border: 'none', padding: '10px 18px', borderRadius: 10, fontFamily: 'inherit', fontSize: 12, fontWeight: 700, display: 'flex', alignItems: 'center', gap: 6, letterSpacing: '0.08em', textTransform: 'uppercase', transition: 'all 0.15s' },
  endWorkoutBtn: { width: '100%', marginTop: 22, background: 'transparent', border: '1px solid', padding: '14px', borderRadius: 12, fontFamily: 'inherit', fontSize: 12, fontWeight: 600, cursor: 'pointer', letterSpacing: '0.08em', textTransform: 'uppercase' },

  // ─── Tip blocks ───
  tipBlock: { marginTop: 28, padding: '16px 18px', background: C.card, border: `1px solid ${C.border}`, borderRadius: 12 },
  tipLabel: { fontSize: 9, color: C.hot, letterSpacing: '0.25em', fontWeight: 700, marginBottom: 8 },
  tipText: { fontSize: 12, color: C.textDim, lineHeight: 1.6 },

  // ─── Modals ───
  modalOverlay: { position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.88)', display: 'flex', alignItems: 'flex-end', justifyContent: 'center', zIndex: 100, backdropFilter: 'blur(8px)' },
  modalBox: { background: C.card, border: `1px solid ${C.border}`, borderRadius: '20px 20px 0 0', padding: 22, width: '100%', maxWidth: 560, maxHeight: '85vh', overflowY: 'auto', boxSizing: 'border-box', boxShadow: '0 -8px 32px rgba(0,0,0,0.5)' },
  modalHeader: { display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 18 },
  modalClose: { background: C.bg, border: `1px solid ${C.border}`, color: C.textDim, width: 34, height: 34, borderRadius: 10, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'inherit' },
  swapItem: { background: C.cardHi, border: '1px solid', borderRadius: 12, padding: '16px 16px', cursor: 'pointer', fontFamily: 'inherit', color: 'inherit', textAlign: 'left', width: '100%' },
  swapMeta: { marginTop: 10 },
  swapMetaLabel: { fontSize: 9, color: C.textMute, letterSpacing: '0.12em', fontWeight: 600, marginBottom: 4 },
  swapMetaText: { fontSize: 12, color: C.textDim, lineHeight: 1.5 },
  scopeBtn: { background: 'transparent', border: '1px solid', padding: '7px 14px', borderRadius: 10, fontFamily: 'inherit', fontSize: 11, cursor: 'pointer', letterSpacing: '0.03em', fontWeight: 500 },
  exportPre: { background: C.bg, border: `1px solid ${C.border}`, padding: 14, borderRadius: 10, fontSize: 10, color: C.textDim, overflowX: 'auto', whiteSpace: 'pre', fontFamily: "'IBM Plex Mono', monospace", maxHeight: '40vh', overflowY: 'auto', margin: 0 },
  copyBtn: { width: '100%', background: C.hot, border: 'none', color: '#1a1410', padding: '14px', borderRadius: 12, fontFamily: 'inherit', fontSize: 13, fontWeight: 700, cursor: 'pointer', marginTop: 14, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8, letterSpacing: '0.06em', textTransform: 'uppercase' },
  cancelBtn: { flex: 1, background: 'transparent', border: `1px solid ${C.border}`, color: C.textDim, padding: '14px', borderRadius: 12, fontFamily: 'inherit', fontSize: 12, fontWeight: 600, cursor: 'pointer', letterSpacing: '0.03em' },
  tutorialBox: { background: C.cardHi, border: `1px solid ${C.border}`, borderRadius: 12, padding: '16px 18px' },
  youtubeBtn: { width: '100%', background: 'transparent', border: `1px solid ${C.hot}`, color: C.hot, padding: '13px', borderRadius: 12, fontFamily: 'inherit', fontSize: 12, fontWeight: 700, cursor: 'pointer', marginTop: 16, textDecoration: 'none', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8, letterSpacing: '0.06em', textTransform: 'uppercase', boxSizing: 'border-box' },
  moodBtn: { background: 'transparent', border: '1px solid', padding: '18px 12px', borderRadius: 12, fontFamily: 'inherit', fontSize: 13, fontWeight: 600, cursor: 'pointer', letterSpacing: '0.03em', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8 },

  // ─── Welcome ───
  welcomeBlock: { display: 'flex', flexDirection: 'column', gap: 14, marginBottom: 24 },
  welcomeStep: { background: C.card, border: `1px solid ${C.border}`, borderRadius: 14, padding: '16px 18px', display: 'flex', gap: 14, alignItems: 'flex-start', boxShadow: '0 3px 10px rgba(0,0,0,0.2)' },
  welcomeNum: { fontSize: 16, fontWeight: 700, color: C.hot, fontFamily: "'Bebas Neue', sans-serif", letterSpacing: '0.04em', flexShrink: 0, paddingTop: 2 },
  welcomeStepTitle: { fontSize: 15, fontWeight: 700, color: C.text, marginBottom: 5 },
  welcomeStepBody: { fontSize: 13, color: C.textDim, lineHeight: 1.6 },
  welcomeCTA: { width: '100%', background: C.hot, border: 'none', color: '#1a1410', padding: '15px', borderRadius: 12, fontFamily: 'inherit', fontSize: 14, fontWeight: 700, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center', letterSpacing: '0.12em', textTransform: 'uppercase' },

  // ─── Analytics / stats ───
  statsGrid: { display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 10 },
  statCard: { background: C.card, border: `1px solid ${C.border}`, borderRadius: 12, padding: '16px 12px', textAlign: 'center', boxShadow: '0 2px 8px rgba(0,0,0,0.15)' },
  statVal: { fontSize: 32, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif", color: C.text, lineHeight: 1 },
  statLabel: { fontSize: 9, color: C.textMute, letterSpacing: '0.18em', marginTop: 8, fontWeight: 600 },
  sectionLabel: { fontSize: 10, color: C.textMute, letterSpacing: '0.22em', fontWeight: 600, marginBottom: 12 },
  emptyState: { background: C.card, border: `1px dashed ${C.border}`, borderRadius: 14, padding: '44px 24px', display: 'flex', flexDirection: 'column', alignItems: 'center', textAlign: 'center', marginTop: 16 },

  // ─── Rest bubble ───
  restOverlay: { position: 'fixed', inset: 0, background: 'rgba(10,8,6,0.92)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 200, padding: 20, boxSizing: 'border-box', backdropFilter: 'blur(8px)' },
  restBubble: { background: C.card, border: `1px solid ${C.border}`, borderRadius: 24, padding: '34px 30px', maxWidth: 340, width: '100%', boxSizing: 'border-box', position: 'relative', display: 'flex', flexDirection: 'column', alignItems: 'center', boxShadow: '0 12px 40px rgba(0,0,0,0.6)' },
  restHideBtn: { position: 'absolute', top: 16, right: 16, background: C.bg, border: `1px solid ${C.border}`, color: C.textDim, padding: '6px 12px', borderRadius: 8, fontFamily: 'inherit', fontSize: 10, cursor: 'pointer', letterSpacing: '0.08em', display: 'flex', alignItems: 'center', gap: 4 },
  restLabel: { fontSize: 11, letterSpacing: '0.35em', fontWeight: 700, marginBottom: 18, marginTop: 6 },
  restRing: { position: 'relative', width: 220, height: 220, marginBottom: 26 },
  restTimeText: { position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' },
  restNumbers: { fontSize: 60, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif", letterSpacing: '0.03em', lineHeight: 1 },
  restSub: { fontSize: 10, color: C.textMute, letterSpacing: '0.22em', marginTop: 10 },
  restControls: { display: 'flex', gap: 8, flexWrap: 'wrap', justifyContent: 'center' },
  restCtrlBtn: { background: C.bg, border: `1px solid ${C.border}`, color: C.textDim, padding: '9px 16px', borderRadius: 10, fontFamily: 'inherit', fontSize: 11, cursor: 'pointer', letterSpacing: '0.06em', textTransform: 'uppercase', display: 'flex', alignItems: 'center', gap: 5, fontWeight: 600 },

  // ─── PR toast ───
  prToast: { position: 'fixed', top: 22, left: '50%', transform: 'translateX(-50%)', background: C.card, border: `1px solid ${C.hot}`, borderRadius: 14, padding: '14px 18px', maxWidth: 340, width: 'calc(100% - 32px)', display: 'flex', alignItems: 'center', gap: 12, zIndex: 300, cursor: 'pointer', boxShadow: '0 12px 32px rgba(0,0,0,0.6), 0 0 0 1px rgba(232, 93, 74, 0.15)' },

  // ─── Summary ───
  summaryStats: { display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 10 },
  summaryStat: { background: C.cardHi, border: `1px solid ${C.border}`, borderRadius: 12, padding: '18px 8px', textAlign: 'center' },
  summaryStatVal: { fontSize: 34, fontWeight: 700, fontFamily: "'Bebas Neue', sans-serif", lineHeight: 1 },
  summaryStatLabel: { fontSize: 9, color: C.textMute, letterSpacing: '0.18em', marginTop: 8, fontWeight: 600 },
  summaryRow: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '10px 14px', background: C.bg, border: `1px solid ${C.border}`, borderRadius: 10 },
  summaryRowLabel: { fontSize: 13, color: C.textDim },
  summaryRowVal: { fontSize: 14, color: C.text, fontWeight: 600 },
  trophyIconBig: { display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 22, background: `radial-gradient(circle, ${C.hot}30 0%, transparent 70%)`, borderRadius: '50%' },
};
