package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Figure
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The Training chapter: seven lessons, read in order, about how lifting actually works.
 *
 * Rewritten 2026-09-26 when 35 pieces were cut to 12. Each lesson here absorbed several old ones
 * (the ids they replaced are in [AcademyRegistry.aliases]) and the four Library articles folded in
 * with their sources. The rule for what stayed: something a lifter can use in the next session, said
 * in everyday words, with the number attached. What the coach does with it gets one short section at
 * the end, never the whole lesson.
 */
internal object AcademyTraining {

    val gettingStronger = Lesson(
        id = "training.getting_stronger",
        track = LessonTrack.TRAINING,
        title = "How you actually get stronger",
        summary = "Repeat the same lifts and ask for a little more each time. Reps first, then weight.",
        unlock = LessonUnlock(
            label = "Open your first program day",
            detail = "Available as soon as there is a plan to open.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "Your body adapts to demands it keeps meeting. Lift the same weight for the same " +
                    "reps forever and there is nothing left to adapt to, so you stay where you are. " +
                    "Ask for slightly more, again and again, and it has to keep catching up."
            ),
            Paragraph(
                "That only works if the lifts come back. A program is a set of days you repeat, so " +
                    "this week's squat can be compared with last week's. Change everything every " +
                    "session and you are exercising, which is fine, but you are not training."
            ),
            Figure(
                "fig.repeat_vs_random",
                "Same lifts every week make progress visible. Random sessions leave nothing to measure against."
            ),
            Heading("Reps first, then weight"),
            Paragraph(
                "The simplest way to ask for more is called double progression. Pick a rep range, " +
                    "say 8 to 10. Add reps each session at the same weight. When every set hits 10, " +
                    "add the smallest jump the bar allows and drop back to 8."
            ),
            Bullets(
                listOf(
                    "Week 1: 60 kg for 8, 8, 8",
                    "Week 3: 60 kg for 10, 10, 9",
                    "Week 4: 60 kg for 10, 10, 10, so the weight goes up",
                    "Week 5: 62.5 kg for 8, 8, 8, and the climb starts again"
                )
            ),
            Paragraph(
                "Every jump in weight is earned by reps you already did, rather than guessed. When " +
                    "progress slows, add a set before you add a new exercise."
            ),
            Heading("It slows down, and that is normal"),
            Paragraph(
                "Early on you can add weight almost every session, because your nervous system is " +
                    "still learning the lift. After a year or two the same climb takes weeks, and later " +
                    "months. Nothing is wrong when that happens. Keep adding reps before weight, and " +
                    "expect the jumps to come further apart."
            ),
            Figure(
                "fig.slowing_gains",
                "Illustrative: the same climb takes longer every year you train."
            ),
            Heading("What the coach does"),
            Paragraph(
                "It runs this for you. It watches each lift's rep range and only suggests more " +
                    "weight once you own the top of it. It needs two sessions of the same lift " +
                    "before it will suggest anything."
            ),
            Callout("Repeatable beats optimal. A plan you follow beats a better one you don't.")
        )
    )

    val effort = Lesson(
        id = "training.effort",
        track = LessonTrack.TRAINING,
        title = "How hard a set should be",
        summary = "Stop most sets one to three reps short of failure. Any rep range from 5 to 30 builds muscle that way.",
        unlock = LessonUnlock(
            label = "Log a set",
            detail = "Start any session and log one working set.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "A set only builds muscle if it gets hard. What makes it hard is not the weight or " +
                    "the rep count, it is how close you finish to the rep you could not do. That " +
                    "distance is called reps in reserve, and it is the most useful number in lifting."
            ),
            Heading("Reps in reserve"),
            Bullets(
                listOf(
                    "3 left: working, but comfortable",
                    "2 left: the usual target for working sets",
                    "1 left: the bar is slowing down",
                    "0 left: failure, the next rep would not move"
                )
            ),
            Paragraph(
                "The app logs this as RPE, which is the same scale counted the other way. RPE 8 means " +
                    "two reps left. RPE 10 means none."
            ),
            Figure(
                "ix.rir",
                "Drag across the drawing to see what each number of reps left in the tank looks like."
            ),
            Heading("Failure is optional"),
            Paragraph(
                "When the total work is equal, sets stopped one to three reps short grow muscle " +
                    "about as well as sets taken to failure. Stop five or more short and growth does " +
                    "fall off. The last rep is the most expensive one: it costs the most recovery and " +
                    "adds the least."
            ),
            Figure(
                "fig.cost_vs_stimulus",
                "Illustrative: each rep closer to failure adds less growth and costs more recovery."
            ),
            Paragraph(
                "Save true failure for the last set of an isolation move, like a curl or a lateral " +
                    "raise. On a heavy squat or deadlift it buys little and costs a lot."
            ),
            Heading("Any rep range works"),
            Paragraph(
                "Muscle grows anywhere from about 5 to 30 reps, as long as the set ends close to " +
                    "failure. Strength is fussier: it follows the heavy weights you practise with. So " +
                    "use heavier sets on the lifts you want strong, and whatever range you enjoy for " +
                    "the rest."
            ),
            Paragraph(
                "Most people underestimate what they have left, and the error runs one way: sets " +
                    "you are sure are two short are often four. Taking a set to real failure now and " +
                    "then, on a safe exercise, teaches you where the line is."
            ),
            Callout("Go to failure to learn where it is, not to live there.")
        ),
        sources = listOf(
            Source(
                authors = "Refalo M, Helms E, Trexler E, Hamilton D, Fyfe J",
                title = "Influence of resistance training proximity-to-failure on skeletal muscle hypertrophy: a systematic review with meta-analysis",
                journal = "Sports Medicine",
                year = 2023
            ),
            Source(
                authors = "Robinson Z, Pelland J, Remmert J, et al.",
                title = "Exploring the dose-response relationship between estimated resistance training proximity to failure, strength gain, and muscle hypertrophy",
                journal = "Sports Medicine",
                year = 2024
            ),
            Source(
                authors = "Zourdos M, Klemp A, Dolan C, et al.",
                title = "Novel resistance training-specific rating of perceived exertion scale measuring repetitions in reserve",
                journal = "Journal of Strength and Conditioning Research",
                year = 2016
            ),
            Source(
                authors = "Schoenfeld B, Grgic J, Van Every D, Plotkin D",
                title = "Loading recommendations for muscle strength, hypertrophy, and local endurance: a re-examination of the repetition continuum",
                journal = "Sports",
                year = 2021
            )
        )
    )

    val volume = Lesson(
        id = "training.volume",
        track = LessonTrack.TRAINING,
        title = "How much work a muscle needs",
        summary = "About 10 hard sets per muscle a week gets most of the growth. Past 20, extra sets mostly buy fatigue.",
        unlock = LessonUnlock(
            label = "When the coach first caps a muscle's weekly sets",
            detail = "It starts from typical numbers and adjusts them as your log shows what you recover from.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Volume means hard sets per muscle per week. It is the closest thing lifting has to " +
                    "a dial: turn it up and you grow more, up to a point."
            ),
            Heading("The shape of the curve"),
            Paragraph(
                "Growth rises with weekly sets, but each extra set adds less than the one before. " +
                    "Around 10 hard sets per muscle a week already gets you most of what is on offer. " +
                    "Between 10 and 20 you still gain, just more slowly. Past about 20, most people " +
                    "are paying in fatigue for very little."
            ),
            Figure(
                "ix.volume",
                "Illustrative curves. Drag across them to see where a week of training lands."
            ),
            Bullets(
                listOf(
                    "Under 5: enough to hold on to what you have",
                    "About 10: most of the growth, for a sensible cost",
                    "10 to 20: more growth, if you recover from it",
                    "Over 20: rarely worth it, and hard to sustain"
                )
            ),
            Paragraph(
                "Count only hard sets, the ones that end a few reps from failure. Warm-ups do not " +
                    "count. A set of rows counts for your back, and half-counts for your biceps."
            ),
            Figure(
                "fig.counting_sets",
                "Compound lifts count toward more than one muscle, but fully only toward the main one."
            ),
            Heading("Start low, add when you stall"),
            Paragraph(
                "If you are new, 6 to 10 hard sets per muscle a week is plenty, because everything is " +
                    "new to your body. Add sets when progress stalls and you are recovering well, two at " +
                    "a time, never by doubling."
            ),
            Heading("Keep opposites even"),
            Paragraph(
                "Pushing and pulling, quads and hamstrings. When one side of a pair gets far more " +
                    "sets than the other for months, the weaker side ends up capping the stronger " +
                    "one. The fix is cheap: a couple of extra sets a week for the side that is behind."
            ),
            Heading("What the coach does"),
            Paragraph(
                "It gives each muscle a floor and a ceiling. They start at typical values, then " +
                    "move with your log: if a muscle got more work and you came out stronger, its " +
                    "ceiling rises. If not, it comes down. It also flags a pair that has drifted apart."
            ),
            Callout("The right volume is the most you can recover from, not the most you can survive.")
        ),
        sources = listOf(
            Source(
                authors = "Schoenfeld B, Ogborn D, Krieger J",
                title = "Dose-response relationship between weekly resistance training volume and increases in muscle mass: a systematic review and meta-analysis",
                journal = "Journal of Sports Sciences",
                year = 2017
            ),
            Source(
                authors = "Baz-Valle E, Balsalobre-Fernandez C, Alix-Fages C, Santos-Concejero J",
                title = "A systematic review of the effects of different resistance training volumes on muscle hypertrophy",
                journal = "Journal of Human Kinetics",
                year = 2022
            ),
            Source(
                authors = "Heaselgrave S, Blacker J, Smeuninx B, McKendry J, Breen L",
                title = "Dose-response relationship of weekly resistance-training volume and frequency on muscular adaptations in trained men",
                journal = "International Journal of Sports Physiology and Performance",
                year = 2019
            )
        )
    )

    val form = Lesson(
        id = "training.form",
        track = LessonTrack.TRAINING,
        title = "Form, range and warming up",
        summary = "Weight only counts for the muscle that moved it. Warm up by ramping into the lift, never by tiring yourself out.",
        unlock = LessonUnlock(
            label = "Start a session with its warm-up",
            detail = "Warm-up sets lead a session unless you hold the start button to skip them.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "The weight is a tool, not the score. If you add plates by cutting the range short " +
                    "or letting other muscles help, the muscle you meant to train gets no more work. " +
                    "You just get a bigger number in the log."
            ),
            Bullets(
                listOf(
                    "Lower the weight under control instead of dropping into it",
                    "Use the full range you can control, especially the stretched part",
                    "Keep the working muscle working, rather than bouncing or swinging",
                    "If your form has to change to finish a rep, that rep was past your limit"
                )
            ),
            Paragraph(
                "The stretched end of a movement matters most. The bottom of a squat, the stretch " +
                    "at the top of a pulldown. Cutting it off is the most common way people train " +
                    "less than they think."
            ),
            Figure(
                "fig.range",
                "The deep, stretched end of a rep is where growth is best. A half rep leaves it out."
            ),
            Heading("Warm up by ramping"),
            Paragraph(
                "The best warm-up for a lift is the lift itself, getting heavier. It warms the " +
                    "muscle and rehearses the movement, so your first working set is a real one."
            ),
            Bullets(
                listOf(
                    "The empty bar, or a light weight, for 8 to 10 easy reps",
                    "About half your working weight for 5",
                    "About 70 percent for 3",
                    "About 85 percent for 1 or 2, then your first working set"
                )
            ),
            Figure(
                "ix.warmup",
                "Drag to set your working weight. Every load is rounded to what you can put on the bar."
            ),
            Paragraph(
                "Keep warm-up reps low and far from failure. Only the first lift of the day needs " +
                    "the full ramp. Later lifts need one or two sets at most."
            ),
            Heading("What the coach does"),
            Paragraph(
                "Warm-up sets lead each session. Tag a session as technique work and the coach " +
                    "leaves it out of its progress reads, so a deliberately light day never looks " +
                    "like a stall."
            ),
            Callout("If your warm-up made you tired, it was training, and it will cost you.")
        )
    )

    val recovery = Lesson(
        id = "training.recovery",
        track = LessonTrack.TRAINING,
        title = "Rest is where you grow",
        summary = "Training is the signal. The growth happens in the two or three days after, and mostly while you sleep.",
        unlock = LessonUnlock(
            label = "Use a rest timer",
            detail = "It starts on its own the moment you log a set in a session.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "A hard session leaves you weaker for a while. Given time, your body rebuilds past " +
                    "where you started, and that overshoot is the progress. Train again too early " +
                    "and you start from a hole. Wait too long and the overshoot fades."
            ),
            Heading("Between sets"),
            Paragraph(
                "On heavy compound lifts, rest two to three minutes. Cut it to one minute and your " +
                    "later sets lose reps. It feels harder, but it is less training. On small " +
                    "isolation moves, a minute to ninety seconds is plenty."
            ),
            Figure(
                "fig.rest_between_sets",
                "Illustrative: the same weight for three sets, with two different rests."
            ),
            Heading("Between sessions"),
            Paragraph(
                "A muscle usually needs 48 to 72 hours before it performs at its best again. That " +
                    "is why training each muscle about twice a week works so well: it fits two " +
                    "recoveries into seven days. Heavier, longer sessions and a physical job push " +
                    "the number up."
            ),
            Figure(
                "fig.twice_a_week",
                "Twice a week fits two full recoveries for each muscle."
            ),
            Paragraph(
                "Recovered means you can match or beat what you did last time, not that you feel no " +
                    "soreness at all. A muscle can be a little sore and still ready."
            ),
            Heading("Sleep"),
            Paragraph(
                "Sleep is the cheapest recovery tool you have. After a short night, strength and " +
                    "the number of reps you can do both drop, and a run of short nights slows " +
                    "muscle gain. On a diet, short sleep makes more of the weight you lose come from " +
                    "muscle instead of fat."
            ),
            Bullets(
                listOf(
                    "Aim for 7 to 9 hours, at roughly the same times each day",
                    "One bad night: train, maybe a little lighter",
                    "Several bad nights: cut a set per exercise until it improves"
                )
            ),
            Heading("What the coach does"),
            Paragraph(
                "It spaces sessions for the same muscle, and it learns your own spacing from how " +
                    "your lifts hold up at different gaps. A rest day is part of the plan, not a day " +
                    "you missed."
            ),
            Callout("You do not grow in the gym. You grow in the days after it.")
        ),
        sources = listOf(
            Source(
                authors = "Craven J, McCartney D, Desbrow B, et al.",
                title = "Effects of acute sleep loss on physical performance: a systematic and meta-analytical review",
                journal = "Sports Medicine",
                year = 2022
            ),
            Source(
                authors = "Knowles O, Drinkwater E, Urwin C, Lamon S, Aisbett B",
                title = "Inadequate sleep and muscle strength: implications for resistance training",
                journal = "Journal of Science and Medicine in Sport",
                year = 2018
            ),
            Source(
                authors = "Nedeltcheva A, Kilkus J, Imperial J, Schoeller D, Penev P",
                title = "Insufficient sleep undermines dietary efforts to reduce adiposity",
                journal = "Annals of Internal Medicine",
                year = 2010
            ),
            Source(
                authors = "Schoenfeld B, Grgic J, Krieger J",
                title = "How many times per week should a muscle be trained to maximize muscle hypertrophy? A systematic review and meta-analysis of studies examining the effects of resistance training frequency",
                journal = "Journal of Sports Sciences",
                year = 2019
            )
        )
    )

    val soreness = Lesson(
        id = "training.soreness",
        track = LessonTrack.TRAINING,
        title = "Soreness or injury",
        summary = "Soreness is dull, spread out and fades as you warm up. Injury is sharp, in one spot, and worse under load.",
        unlock = LessonUnlock(
            label = "Flag soreness or illness",
            detail = "From the daily check-in, or by logging a rest day as sick.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "Soreness peaks a day or two after training, then fades over the next few days. It " +
                    "feels dull, spreads across the whole muscle, turns up on both sides evenly, " +
                    "and eases once you get moving."
            ),
            Paragraph(
                "An injury feels different. Sharp instead of dull, in one spot, often near a joint, " +
                    "and worse the more load you put on it. It does not fade on the usual schedule."
            ),
            Figure(
                "fig.pain_check",
                "When in doubt, start with how it feels."
            ),
            Heading("What to do"),
            Bullets(
                listOf(
                    "Sore: train it lighter, or train something else, and it settles",
                    "Sharp, or right on a joint: stop loading it and flag it",
                    "Pain that lasts more than a week or two, or wakes you up: see a professional"
                )
            ),
            Heading("Soreness is not a score"),
            Paragraph(
                "Being sore does not prove a session worked, and not being sore does not mean it " +
                    "failed. You get less sore on movements you repeat, even while you keep " +
                    "progressing. This is called the repeated bout effect. New exercises and long stretched positions make you the most sore."
            ),
            Figure(
                "fig.repeated_bout",
                "Illustrative: the same workout leaves you less sore each time you repeat it."
            ),
            Heading("What the coach does"),
            Paragraph(
                "Flag a muscle as sore and the coach eases its work for a day or two. Flag an " +
                    "injury and it routes around the movement entirely until you clear it."
            ),
            Callout("Soreness is information. It is not a grade.")
        )
    )

    val protein = Lesson(
        id = "training.protein",
        track = LessonTrack.TRAINING,
        title = "Eating for muscle",
        summary = "About 1.6 g of protein per kg of bodyweight a day covers almost everyone. On a diet, aim a little higher.",
        unlock = LessonUnlock(
            label = "When a lift holds while you're losing weight",
            detail = "The coach explains why it refuses to call that a plateau.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Training tells your body to build. Protein is what it builds with. Eat too little " +
                    "and you leave growth on the table however well you train."
            ),
            Heading("How much"),
            Paragraph(
                "The benefit rises until about 1.6 g per kg of bodyweight per day, then flattens. " +
                    "For an 80 kg lifter that is about 130 g. Going up to 2.2 g per kg is a sensible " +
                    "margin, and more than that is not harmful, just not useful."
            ),
            Bullets(
                listOf(
                    "Split it over 3 or 4 meals of roughly 30 to 40 g each",
                    "The total for the day matters far more than the timing",
                    "Food is enough. Powder is a convenience, not a requirement"
                )
            ),
            Figure(
                "ix.protein",
                "Drag to set your bodyweight. Your daily target, split over four meals."
            ),
            Heading("On a diet"),
            Paragraph(
                "When you eat less than you burn, protein protects muscle. Aim toward the top of " +
                    "the range, around 2 g per kg, and keep lifting heavy."
            ),
            Paragraph(
                "Expect your lifts to slip a little while you lose weight. There is less fuel to " +
                    "train on and less to recover with. So a lift that holds steady while your weight " +
                    "drops is a win: you are moving the same load with less of you."
            ),
            Figure(
                "fig.cut_hold",
                "While cutting, a lift that holds flat is beating the usual slide."
            ),
            Heading("What the coach does"),
            Paragraph(
                "It reads your weigh-ins as a trend. While the trend is down, it stops treating a " +
                    "flat lift as a stall. It still speaks up if a lift actually drops, and the " +
                    "normal rules come back when your weight levels out."
            ),
            Callout("While you are cutting, judge a lift by what it holds, not by what it adds.")
        ),
        sources = listOf(
            Source(
                authors = "Morton R, Murphy K, McKellar S, et al.",
                title = "A systematic review, meta-analysis and meta-regression of the effect of protein supplementation on resistance training-induced gains in muscle mass and strength in healthy adults",
                journal = "British Journal of Sports Medicine",
                year = 2018
            ),
            Source(
                authors = "Tagawa R, Watanabe D, Ito K, et al.",
                title = "Dose-response relationship between protein intake and muscle mass increase: a systematic review and meta-analysis of randomized controlled trials",
                journal = "Nutrition Reviews",
                year = 2021
            ),
            Source(
                authors = "Helms E, Zinn C, Rowlands D, Brown S",
                title = "A systematic review of dietary protein during caloric restriction in resistance trained lean athletes: a case for higher intakes",
                journal = "International Journal of Sport Nutrition and Exercise Metabolism",
                year = 2014
            ),
            Source(
                authors = "Schoenfeld B, Aragon A",
                title = "How much protein can the body use in a single meal for muscle-building",
                journal = "Journal of the International Society of Sports Nutrition",
                year = 2018
            )
        )
    )

    /** The chapter in reading order. It doubles as the cold-start curriculum. */
    val ordered: List<Lesson> = listOf(
        gettingStronger,
        effort,
        volume,
        form,
        recovery,
        soreness,
        protein
    )
}
