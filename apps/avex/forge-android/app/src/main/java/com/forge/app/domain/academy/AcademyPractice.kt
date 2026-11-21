package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The second half of the Training chapter: five lessons for the things that come up once the basics
 * are in place. Added 2026-11-18.
 *
 * The first seven ([AcademyTraining]) are read in order and double as the cold-start curriculum, so
 * they stay exactly seven. These are not part of that track: each stands alone and is worth reading
 * when its subject arrives (a lift you want to swap, a stall, a break, a cut, a body part you want
 * out of the plan or brought up). Same rule as the first seven for what earns a place: something a
 * lifter can use in the next session, in everyday words, with the number attached, and what the
 * coach does with it in one short section at the end.
 *
 * Every citation here was checked against the paper itself (authors, title, journal, year) on the
 * day it was written. Where a study's sample limits what it can say, the lesson says so.
 */
internal object AcademyPractice {

    val swaps = Lesson(
        id = "training.swaps",
        track = LessonTrack.TRAINING,
        title = "Swapping one lift for another",
        summary = "Two lifts are interchangeable when they train the same muscle through the same kind of movement. Swap for a reason, then stay put.",
        unlock = LessonUnlock(
            label = "Swap a lift",
            detail = "Use Swap on any exercise in a session.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "No single exercise is required. A muscle responds to being loaded hard through a " +
                    "long range, and several movements can do that. What matters is knowing which " +
                    "swaps keep the session's job intact and which quietly change it."
            ),
            Heading("What makes two lifts interchangeable"),
            Bullets(
                listOf(
                    "The same main muscle. A row and a pulldown both train your back.",
                    "The same kind of movement. A press replaces a press, a squat replaces a squat, a curl replaces a curl.",
                    "The same role in the day. A heavy compound replaces a heavy compound. A fly cannot stand in for a bench press."
                )
            ),
            Paragraph(
                "Match all three and you can swap freely: a dumbbell press for a barbell press, a " +
                    "leg press for a squat on a day your back is tired. Match only the muscle and you " +
                    "have changed what the slot is for."
            ),
            Heading("How often to change"),
            Paragraph(
                "Less often than it feels like. In one eight-week trial, trained men whose exercises " +
                    "were shuffled at random every session gained about as much size and strength as " +
                    "men who kept the same lifts, and reported more motivation to train. But a review " +
                    "of eight studies found that constant, random variation may hold gains back, " +
                    "while a little planned variation helps. Both were in young men, so treat the " +
                    "detail as a guide."
            ),
            Paragraph(
                "The practical reading: you can only add reps to a lift you repeat. Keep a movement " +
                    "for at least several weeks, long enough to see it climb, and change it when you " +
                    "have a reason."
            ),
            Heading("Good reasons to swap"),
            Bullets(
                listOf(
                    "It hurts in a joint, not just a muscle.",
                    "The equipment is taken or you are training somewhere else.",
                    "It has stalled for weeks and a close variation gives you somewhere new to climb.",
                    "You dread it enough that you skip it."
                )
            ),
            Heading("What the coach does"),
            Paragraph(
                "The swap list for a slot only offers movements for the same muscle, and puts the " +
                    "ones of the same kind first. Your history follows the lift you actually did, so " +
                    "a swap starts its own record instead of muddying the old one."
            ),
            Callout("Swap for a reason, then stay with it long enough to see it move.")
        ),
        sources = listOf(
            Source(
                authors = "Baz-Valle E, Schoenfeld B, Torres-Unda J, et al.",
                title = "The effects of exercise variation in muscle thickness, maximal strength and motivation in resistance trained men",
                journal = "PLoS ONE",
                year = 2019
            ),
            Source(
                authors = "Kassiano W, Nunes J, Costa B, et al.",
                title = "Does varying resistance exercises promote superior muscle hypertrophy and strength gains? A systematic review",
                journal = "Journal of Strength and Conditioning Research",
                year = 2022
            )
        )
    )

    val plateaus = Lesson(
        id = "training.plateaus",
        track = LessonTrack.TRAINING,
        title = "When a lift stops moving",
        summary = "One flat session is noise. Three is a stall, and the fix depends on how hard those sessions were.",
        unlock = LessonUnlock(
            label = "When a lift stalls",
            detail = "The coach flags a lift that has not moved across several sessions.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Progress is never a straight line. A bad night, a long day or a missed meal can " +
                    "cost you a rep, and it comes back next time. A stall is something else: the " +
                    "same weight for the same reps, session after session, with nothing in your week " +
                    "to explain it."
            ),
            Heading("First, is it really stalled"),
            Bullets(
                listOf(
                    "Fewer than three sessions flat is not a stall yet.",
                    "Losing weight on purpose changes the question. Holding a lift while you get lighter is progress.",
                    "A lift you do last in a long session is being measured tired. Compare it with itself, not with the first lift of the day."
                )
            ),
            Heading("Then, how hard was it"),
            Paragraph(
                "The same flat line means two opposite things, and effort is what tells them apart."
            ),
            Bullets(
                listOf(
                    "Flat and easy: you had reps in hand. You are not stuck, you are coasting. Push the next sets closer to your limit.",
                    "Flat and hard: every set was near its limit. Pushing harder is what you have been doing. Take about 10 percent off, build back up through the rep range, and you will usually pass the old mark."
                )
            ),
            Paragraph(
                "This is why the effort question after your last set matters. Without it the coach " +
                    "cannot tell coasting from grinding."
            ),
            Heading("If backing off does not do it"),
            Bullets(
                listOf(
                    "Check sleep and food before anything in the gym. Most long stalls start outside it.",
                    "Add a set to the lift, not a new exercise.",
                    "Move it earlier in the session.",
                    "Swap to a close variation and climb that for a few weeks.",
                    "If several lifts stall together, that is fatigue, and the answer is an easier week."
                )
            ),
            Heading("What the coach does"),
            Paragraph(
                "It runs this ladder for you. It waits for a real stall, reads your effort, and " +
                    "then proposes the smallest step that fits: a reset, a set, a rep range, a swap. " +
                    "When several lifts stall at once it reads that as fatigue and proposes a deload " +
                    "instead. Then it watches for two weeks to see whether the change worked."
            ),
            Callout("A stall at easy effort needs a push. A stall at hard effort needs a step back.")
        )
    )

    val comingBack = Lesson(
        id = "training.coming_back",
        track = LessonTrack.TRAINING,
        title = "Coming back after time off",
        summary = "You lose less than you fear and regain it faster than you built it. Start lighter and let it come back.",
        unlock = LessonUnlock(
            label = "When you return from a break",
            detail = "Opens the first session back after a gap in training.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "A week or two away costs almost nothing. Longer breaks do cost something, and how " +
                    "much depends on how long: a review of 103 studies found strength falls further " +
                    "the longer training stops, and falls more in older people. The useful part is " +
                    "what happens next."
            ),
            Heading("It comes back faster than it came"),
            Paragraph(
                "In one six-month study, men who trained for six weeks, stopped for three, and " +
                    "repeated that cycle ended with about the same muscle and strength as men who " +
                    "never stopped. They lost a little in each break and regained it quickly once " +
                    "they were back. It was a small study, 14 young men, so the exact numbers will " +
                    "not be yours, but the pattern is a common one: rebuilding is quicker than " +
                    "building."
            ),
            Heading("The first week back"),
            Bullets(
                listOf(
                    "Start around 10 percent under where you left off. More if the break was months.",
                    "Keep the same lifts. This is not the week to change the plan.",
                    "Stop a rep or two earlier than usual. You will be sorer than the weights suggest.",
                    "Expect to be back at your old numbers within a few weeks, not a few sessions."
                )
            ),
            Paragraph(
                "The mistake is loading the bar to where it was on your first day back. The " +
                    "strength returns before your joints and tendons are ready to be asked for it."
            ),
            Heading("One lift, not the whole plan"),
            Paragraph(
                "The same applies to a single lift you have not done in weeks, even if you never " +
                    "stopped training: a movement that rotated out, or a body part you had skipped. " +
                    "It starts a little under where you left it."
            ),
            Heading("What the coach does"),
            Paragraph(
                "It notices the gap. Your first sessions back open with loads held about 10 percent " +
                    "under, and it says why on the day. A lift you have not trained in three weeks or " +
                    "more gets the same ease on its own, and nothing counts against your record " +
                    "while you ramp."
            ),
            Callout("Start lighter than your pride wants. It comes back faster than it came.")
        ),
        sources = listOf(
            Source(
                authors = "Bosquet L, Berryman N, Dupuy O, et al.",
                title = "Effect of training cessation on muscular performance: a meta-analysis",
                journal = "Scandinavian Journal of Medicine and Science in Sports",
                year = 2013
            ),
            Source(
                authors = "Ogasawara R, Yasuda T, Ishii N, Abe T",
                title = "Comparison of muscle hypertrophy following 6-month of continuous and periodic strength training",
                journal = "European Journal of Applied Physiology",
                year = 2013
            )
        )
    )

    val losingWeight = Lesson(
        id = "training.losing_weight",
        track = LessonTrack.TRAINING,
        title = "Lifting while you lose weight",
        summary = "Keep the weight on the bar and lose slowly. Holding your lifts while you get lighter is the win.",
        unlock = LessonUnlock(
            label = "When your weight trends down",
            detail = "Opens once your logged bodyweight has been falling for a few weeks.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Eating less than you burn makes muscle harder to build and easier to lose. Lifting " +
                    "is what tells your body which tissue to keep. So the job while losing weight " +
                    "changes: you are no longer trying to add, you are trying to hold."
            ),
            Heading("What a deficit costs"),
            Paragraph(
                "A review of the trials found that people lifting in a calorie deficit gained less " +
                    "lean mass than people eating enough, while their strength gains were about the " +
                    "same. A deficit of roughly 500 calories a day was enough to stop lean mass from " +
                    "increasing. Most of those studies were in people new to lifting, who gain " +
                    "strength quickly whatever they eat, so the picture for experienced lifters is " +
                    "less settled."
            ),
            Heading("How fast to lose"),
            Paragraph(
                "Slower keeps more. In a study of athletes who all kept lifting four times a week, " +
                    "those losing about 0.7 percent of their bodyweight a week kept and even added " +
                    "lean mass, and those losing faster did not. At a bodyweight of {w:80|180} that " +
                    "is around {w:0.6|1.3} a week."
            ),
            Heading("What to keep and what to cut"),
            Bullets(
                listOf(
                    "Keep the weight on the bar. Heavy sets are the signal to hold muscle.",
                    "If something has to give, give up a set, not the load.",
                    "Keep protein high. The eating lesson has the number.",
                    "Expect your reps to dip late in a cut. That is the deficit, not a failing plan."
                )
            ),
            Heading("What the coach does"),
            Paragraph(
                "When your logged bodyweight has been falling, it stops treating a flat lift as a " +
                    "stall and holds its plateau calls. A lift that stays put while you get lighter " +
                    "is doing its job, and it will say so instead of asking for more."
            ),
            Callout("While you lose weight, a lift that holds is a lift that won.")
        ),
        sources = listOf(
            Source(
                authors = "Murphy C, Koehler K",
                title = "Energy deficiency impairs resistance training gains in lean mass but not strength: a meta-analysis and meta-regression",
                journal = "Scandinavian Journal of Medicine and Science in Sports",
                year = 2022
            ),
            Source(
                authors = "Garthe I, Raastad T, Refsnes P, et al.",
                title = "Effect of two different weight-loss rates on body composition and strength and power-related performance in elite athletes",
                journal = "International Journal of Sport Nutrition and Exercise Metabolism",
                year = 2011
            )
        )
    )

    val bodyParts = Lesson(
        id = "training.body_parts",
        track = LessonTrack.TRAINING,
        title = "Skipping, holding or bringing up a body part",
        summary = "Skip takes a muscle out. Light holds it with very little. Focus brings it up by giving it more and everything else less.",
        unlock = LessonUnlock(
            label = "Set a body part's level",
            detail = "Settings, Program, Body parts.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "A plan does not have to train everything equally. An injury, a sport, or simply " +
                    "what you want from training are all good reasons to leave a part out, keep it " +
                    "ticking over, or put it first for a while."
            ),
            Heading("Skip: what it costs"),
            Paragraph(
                "A muscle you stop training gets smaller and weaker, slowly at first and then " +
                    "steadily. That is the whole cost, and it is a fair trade if you have decided " +
                    "on it. It comes back faster than it was built when you return to it."
            ),
            Heading("Light: holding takes very little"),
            Paragraph(
                "Keeping what you have needs far less than building it. In a study of 70 adults, " +
                    "younger people who cut their training to as little as one ninth of what built " +
                    "their muscle kept its size for 32 weeks. Older adults needed more to hold size, " +
                    "though their strength held. A later review put the floor for younger people at " +
                    "about one session a week with one set per exercise, as long as the set stays " +
                    "heavy."
            ),
            Paragraph(
                "So Light is one slot a week at two sets. Keep the weight where it was. The effort " +
                    "is what holds it, not the volume."
            ),
            Heading("Focus: more for one means less for the rest"),
            Bullets(
                listOf(
                    "The muscle is trained more often, up to three times a week.",
                    "It goes first in its sessions, while you are fresh.",
                    "Everything else gives up a little, so the week does not get longer.",
                    "Two muscles at most. Focus on five things and you have focused on none.",
                    "It runs six weeks, then the plan goes back to normal. A muscle cannot be pushed harder than the rest forever."
                )
            ),
            Heading("What the coach does"),
            Paragraph(
                "A skipped muscle leaves the plan and stays out through every rebuild and " +
                    "rotation. The coach stops proposing work for it and stops counting it in any " +
                    "balance check: you will not be told you are neglecting something you chose to " +
                    "leave. A skip with an end date brings the muscle back on that day, with lighter " +
                    "loads first."
            ),
            Callout("Building takes a lot. Holding takes very little. Use the difference.")
        ),
        sources = listOf(
            Source(
                authors = "Bickel C, Cross J, Bamman M",
                title = "Exercise dosing to retain resistance training adaptations in young and older adults",
                journal = "Medicine and Science in Sports and Exercise",
                year = 2011
            ),
            Source(
                authors = "Spiering B, Mujika I, Sharp M, Foulis S",
                title = "Maintaining physical performance: the minimal dose of exercise needed to preserve endurance and strength over time",
                journal = "Journal of Strength and Conditioning Research",
                year = 2021
            )
        )
    )

    /** In the order the contents page lists them, after the first seven. */
    val ordered: List<Lesson> = listOf(swaps, plateaus, comingBack, losingWeight, bodyParts)
}
