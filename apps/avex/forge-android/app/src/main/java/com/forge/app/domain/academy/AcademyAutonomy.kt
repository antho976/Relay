package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The trust and autonomy lessons (Coach v3 E) — C5 and C6, shipped exactly when autonomy ships.
 *
 * These are the "tool, not strangle" lessons. If the coach is going to start doing things on its
 * own, the user is owed a plain account of what it may do, on what evidence, and how to take any of
 * it back.
 */
internal object AcademyAutonomy {

    val trustTiers = Lesson(
        id = "coach.trust_tiers",
        track = LessonTrack.COACH,
        title = "What each trust tier means",
        summary = "Five levels, earned from outcomes. You can cap it anywhere, any time.",
        unlock = LessonUnlock(
            label = "When the coach's trust tier moves",
            detail = "Tiers are earned from outcomes, and you can cap them in Settings at any time.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The coach's authority is earned rather than assumed. It starts by watching, and " +
                    "each level up is bought with judged results on your own training."
            ),
            Heading("The five levels"),
            Bullets(
                listOf(
                    "Observing: it watches and says nothing",
                    "Proposing: it suggests, you decide every change",
                    "Auto-applying: change types with a track record apply themselves, always undoable",
                    "Proactive: it plans blocks and starts projects, announcing them first",
                    "Autonomous: it owns the programme, acts first and tells you after"
                )
            ),
            Heading("How it moves"),
            Paragraph(
                "Up is a sustained win rate across enough judged calls, plus time. Down is a " +
                    "sustained bad run or repeated reverts by you, never a single miss. That gap " +
                    "matters: a coach that fell a level every time one call missed would spend its " +
                    "life oscillating instead of coaching."
            ),
            Paragraph(
                "The last level is different. Reaching it only earns the coach an offer. Full " +
                    "autonomy turns on when you say so, and never before."
            ),
            Callout("Trust the automation exactly as much as its record earns. The record is on the Coach page.")
        )
    )

    val takingDecisionsBack = Lesson(
        id = "coach.taking_decisions_back",
        track = LessonTrack.COACH,
        title = "How to take any decision back",
        summary = "Every act is watched, undoable, and cappable. The whole point is that you can stop using it.",
        unlock = LessonUnlock(
            label = "The first time the coach acts on its own",
            detail = "Only ever after you've raised its tier that far.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Everything the coach does is written through the same paths you use, which means " +
                    "everything it does can be undone the same way you would undo it."
            ),
            Bullets(
                listOf(
                    "Any single change: undo it from the Coach page",
                    "A whole class of change: cap the tier in Settings and it stops",
                    "Structural changes past their undo window: the coach rebuilds the old shape and keeps every logged session",
                    "Your own manual edits always win, and become constraints the coach plans around"
                )
            ),
            Heading("Reading its record"),
            Paragraph(
                "Each applied change carries a verdict once its window closes: it worked, it " +
                    "didn't, or you weren't there for it. The third is not counted against the " +
                    "coach, because a window you spent ill or away says nothing about the advice."
            ),
            Callout(
                "The goal of this whole system is that you could do it yourself. Using the coach is " +
                    "a convenience, not a dependency."
            )
        )
    )

    val all: List<Lesson> = listOf(trustTiers, takingDecisionsBack)
}
