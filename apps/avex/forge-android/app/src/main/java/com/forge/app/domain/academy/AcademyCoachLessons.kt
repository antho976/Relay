package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Example
import com.forge.app.domain.academy.LessonBlock.Figure
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The Your coach chapter: three lessons on what the coach decides and how to overrule it.
 *
 * Rewritten 2026-09-26. Fourteen machinery lessons (trust tiers, block cards, projects, HRV, goal
 * conflicts...) folded into three, because a reader needs to know how the coach thinks, not a manual
 * for every panel. Each still says what the coach actually does, so a reader could overrule it.
 */
internal object AcademyCoachLessons {

    val howItDecides = Lesson(
        id = "coach.how_it_decides",
        track = LessonTrack.COACH,
        title = "How the coach decides",
        summary = "It reads your log, proposes a change, and checks two weeks later whether it worked. You can undo anything.",
        unlock = LessonUnlock(
            label = "When your first week brief lands",
            detail = "The coach writes one after a week with training in it.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Every Monday the coach runs the same loop. It reads what you logged, proposes a " +
                    "few changes, waits for you to take or skip each one, and two weeks later judges " +
                    "whether each change actually worked."
            ),
            Bullets(
                listOf(
                    "Nothing changes without a reason you can read",
                    "Any change undoes in one tap",
                    "Every applied change gets a verdict after its two-week watch",
                    "When it has too little to go on, it says nothing rather than guess"
                )
            ),
            Figure(
                "fig.one_change",
                "The life of one change, from proposal to verdict."
            ),
            Heading("It runs on your log"),
            Paragraph(
                "The coach cannot see you lift. Everything it believes comes from the weight, the " +
                    "reps and the effort you logged. A rep you did not really finish becomes a target " +
                    "you cannot hit. An effort rating lower than the truth makes the next jump too big."
            ),
            Paragraph(
                "Rating effort takes practice, and early guesses are fine. Guess the same way every " +
                    "time and the trend is still useful."
            ),
            Heading("Trust is earned"),
            Paragraph(
                "At first it only proposes. The kinds of change that keep working earn the right to " +
                    "apply themselves, one type at a time, and every one of those still undoes. A " +
                    "bad run, or you undoing its work, takes that right away again. Never a single miss."
            ),
            Figure(
                "fig.trust",
                "Illustrative records. Each kind of change earns trust on its own."
            ),
            Heading("Taking it back"),
            Bullets(
                listOf(
                    "One change: undo it from the Coach page",
                    "A whole kind of change: cap how much it may do in Settings",
                    "Your own edits always win, and become rules it plans around"
                )
            ),
            Callout("The point is that you could do all of this yourself. The coach just saves you the time.")
        )
    )

    val readiness = Lesson(
        id = "coach.readiness",
        track = LessonTrack.COACH,
        title = "What readiness means",
        summary = "A small nudge to today's targets from your sleep, your check-in and your own normal. It never cancels a session.",
        unlock = LessonUnlock(
            label = "When a readiness score first appears",
            detail = "Needs recovery data: a check-in answer, or sleep and resting heart rate from a watch.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Readiness answers one question: how much should today ask of you? It is a short " +
                    "list of readings, each worth a point or two, added up. The total is capped, so " +
                    "it can only move today's targets by a few percent."
            ),
            Bullets(
                listOf(
                    "Your morning check-in: sleep, soreness, stress and drive",
                    "Last night's sleep, when a watch reports it",
                    "Resting heart rate against your own normal",
                    "Heart-rate variability, when a watch records it overnight",
                    "Recent lifting and cardio, which draw on the same recovery",
                    "Anything you flagged: illness, injury, time away"
                )
            ),
            Example(
                key = "readiness_today",
                label = "Today",
                fallback = "log a few sessions with a check-in and today's number shows here"
            ),
            Figure(
                "fig.readiness_sum",
                "Illustrative: a handful of readings adding up to one small nudge."
            ),
            Heading("Your normal, not an average"),
            Paragraph(
                "Every reading is compared with your own recent baseline. A resting heart rate of " +
                    "58 means nothing on its own. Five beats above your usual does."
            ),
            Paragraph(
                "Heart-rate variability jumps around from night to night. One low night is mostly " +
                    "noise, so the coach compares the last night or two with your last two weeks, " +
                    "and a real drop costs a single point."
            ),
            Figure(
                "fig.hrv_noise",
                "Illustrative: one low night is noise. A run of them is a signal."
            ),
            Heading("Your answers count"),
            Paragraph(
                "In the research, how you say you feel holds up at least as well as what a watch " +
                    "measures. You often know before your watch does. No watch at all changes nothing " +
                    "about how you are coached."
            ),
            Callout("A bad night is a reason to train a little lighter, not a reason to skip.")
        ),
        sources = listOf(
            Source(
                authors = "Saw A, Main L, Gastin P",
                title = "Monitoring the athlete training response: subjective self-reported measures trump commonly used objective measures: a systematic review",
                journal = "British Journal of Sports Medicine",
                year = 2016
            ),
            Source(
                authors = "Plews D, Laursen P, Stanley J, Kilding A, Buchheit M",
                title = "Training adaptation and heart rate variability in elite endurance athletes: opening the door to effective monitoring",
                journal = "Sports Medicine",
                year = 2013
            )
        )
    )

    val blocks = Lesson(
        id = "coach.blocks",
        track = LessonTrack.COACH,
        title = "Blocks, deloads and one goal at a time",
        summary = "A few weeks aimed at one thing, then an easy week to cash it in. Goals that compete get taken in turn.",
        unlock = LessonUnlock(
            label = "When your first block starts",
            detail = "The coach plans one once it can see far enough ahead to commit to a few weeks.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Going hard forever does not fail straight away. It fails slowly. Tiredness builds " +
                    "faster than fitness, so your numbers drift down while sessions feel harder. A " +
                    "block is the fix: a run of weeks that build up, then back off."
            ),
            Heading("Four phases"),
            Bullets(
                listOf(
                    "Build: sets go up at moderate effort, where most of the growth comes from",
                    "Intensify: fewer sets, heavier weights",
                    "Peak: volume drops so you arrive fresh enough to lift near your best",
                    "Deload: an easy week before the next block starts higher"
                )
            ),
            Figure(
                "fig.volume_vs_weight",
                "Across a block, sets and weight trade places, then both ease off."
            ),
            Heading("Why the easy week works"),
            Paragraph(
                "Tiredness clears faster than fitness fades. Cut the work for a week and the " +
                    "tiredness goes while the fitness stays, so what you built finally shows. The " +
                    "research on deload weeks as such is thin. What is well supported is that easing " +
                    "off before a test improves it."
            ),
            Paragraph(
                "The coach plans one deload per block, and pulls it forward if fatigue builds early."
            ),
            Heading("One goal at a time"),
            Paragraph(
                "Some goals sit together fine: a bench target, turning up three times a week, some " +
                    "easy cardio. Others fight. Losing fat and setting a new best both draw on the same " +
                    "recovery, so chasing both at once usually means doing neither well."
            ),
            Figure(
                "fig.goals_in_turn",
                "Two goals that share your recovery go better one after the other."
            ),
            Paragraph(
                "When two of your goals compete, the coach suggests an order and holds the other at " +
                    "maintenance. It also picks one project at a time, like a lagging muscle or a " +
                    "gap between opposite muscles, with a reason and a finish line. You can end a " +
                    "block or drop a project whenever it stops fitting your life."
            ),
            Callout("Taking goals in turn is not giving one up. It is refusing to half-do both.")
        ),
        sources = listOf(
            Source(
                authors = "Bell L, Nolan D, Immonen V, et al.",
                title = "You can't shoot another bullet until you've reloaded the gun: coaches' perceptions, practices and experiences of deloading in strength and physique sports",
                journal = "Frontiers in Sports and Active Living",
                year = 2022
            ),
            Source(
                authors = "Pritchard H, Keogh J, Barnes M, McGuigan M",
                title = "Effects and mechanisms of tapering in maximizing muscular strength",
                journal = "Strength and Conditioning Journal",
                year = 2015
            )
        )
    )

    val ordered: List<Lesson> = listOf(howItDecides, readiness, blocks)
}

/**
 * The Cardio chapter: two lessons on conditioning that serves lifting.
 *
 * Six engine lessons folded into two: the easy base (zone 2, heart rate, measuring it without a lab)
 * and the hard end (intervals, and where to put them so they don't cost your lifting).
 */
internal object AcademyCardio {

    val zone2 = Lesson(
        id = "cardio.zone2",
        track = LessonTrack.CARDIO,
        title = "Easy cardio, and why lifters need it",
        summary = "The hardest pace you can hold while still talking in full sentences. It helps you recover between sets.",
        unlock = LessonUnlock(
            label = "When the coach first prescribes cardio",
            detail = "Conditioning enters the plan once there's a lifting week to build it around.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Between sets, your body clears what the last set left behind and refuels for the " +
                    "next. That job runs on your aerobic system. The better it is, the more of your " +
                    "third and fourth sets is left."
            ),
            Bullets(
                listOf(
                    "Faster recovery between sets, so later sets hold up",
                    "Faster recovery between sessions, so you can handle more volume",
                    "A lower resting heart rate, which is worth having anyway"
                )
            ),
            Heading("Zone 2 is the talk test"),
            Paragraph(
                "Easy cardio, often called zone 2, is the pace you could hold for an hour while " +
                    "still talking in full sentences. If you can only get out a few words at a time, " +
                    "slow down. It feels too easy, and that is the point: it builds the base without " +
                    "costing the recovery your lifting needs."
            ),
            Figure(
                "fig.talk_test",
                "The talk test is accurate enough to train by. No watch needed."
            ),
            Paragraph(
                "Two or three sessions of 30 to 45 minutes a week is plenty. Walking uphill, " +
                    "cycling or rowing all count."
            ),
            Heading("If you wear a heart-rate monitor"),
            Paragraph(
                "Zones come from your own maximum and resting heart rate, not a chart on the gym " +
                    "wall. Heart rate creeping up while your pace stays the same is called drift, and " +
                    "it usually means heat, not enough fluid, or just time on your feet."
            ),
            Heading("Knowing it's working"),
            Bullets(
                listOf(
                    "The same route at the same effort gets faster",
                    "Your heart rate climbs less during a steady session",
                    "Your resting heart rate trends down over weeks"
                )
            ),
            Figure(
                "fig.base_building",
                "Illustrative: the same easy effort carries you further as your base improves."
            ),
            Callout("Cardio is there to support your lifting. It should never take over your week.")
        ),
        sources = listOf(
            Source(
                authors = "Seiler S",
                title = "What is best practice for training intensity and duration distribution in endurance athletes?",
                journal = "International Journal of Sports Physiology and Performance",
                year = 2010
            )
        )
    )

    val intervals = Lesson(
        id = "cardio.intervals",
        track = LessonTrack.CARDIO,
        title = "Hard cardio without losing strength",
        summary = "One interval session a week is enough. Keep it away from heavy leg days and your lifting barely notices.",
        unlock = LessonUnlock(
            label = "When intervals are first prescribed",
            detail = "Rationed on top of your base, never the default answer.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Intervals, short hard efforts with rest in between, are the fastest way to raise " +
                    "your top-end fitness. They also cost real recovery, out of the same pot your " +
                    "lifting draws on."
            ),
            Heading("The interference effect"),
            Paragraph(
                "Hard endurance work alongside lifting can blunt strength and power gains. The " +
                    "detail that usually gets dropped is where it bites: explosive lifts, long runs, " +
                    "and hard cardio right before heavy legs. Spaced sensibly, the cost is small."
            ),
            Figure(
                "fig.interference",
                "Illustrative ranking of how much each kind of cardio gets in the way of lifting."
            ),
            Bullets(
                listOf(
                    "One hard session a week, on top of an easy base you already have",
                    "At least a day away from heavy squats or deadlifts",
                    "Cycling gets in the way of leg strength less than running does",
                    "Warm up and cool down, both are part of the session",
                    "Drop intervals completely in a deload week"
                )
            ),
            Paragraph(
                "Easy cardio costs almost nothing and can go almost anywhere in the week. Only the " +
                    "hard work needs placing."
            ),
            Heading("What the coach does"),
            Paragraph(
                "It places hard sessions away from heavy leg days, and takes yesterday's cardio out " +
                    "of today's readiness once. If the two start competing, it moves a session."
            ),
            Callout("Where you put it is most of the answer. You rarely have to pick one or the other.")
        ),
        sources = listOf(
            Source(
                authors = "Schumann M, Feuerbacher J, Sunkeler M, et al.",
                title = "Compatibility of concurrent aerobic and strength training for skeletal muscle size and function: an updated systematic review and meta-analysis",
                journal = "Sports Medicine",
                year = 2022
            ),
            Source(
                authors = "Wilson J, Marin P, Rhea M, et al.",
                title = "Concurrent training: a meta-analysis examining interference of aerobic and resistance exercises",
                journal = "Journal of Strength and Conditioning Research",
                year = 2012
            )
        )
    )

    val ordered: List<Lesson> = listOf(zone2, intervals)
}
