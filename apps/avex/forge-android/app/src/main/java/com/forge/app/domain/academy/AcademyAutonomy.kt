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
        summary = "Five levels, earned from results. You can cap it anywhere, at any time.",
        unlock = LessonUnlock(
            label = "When the coach's trust tier moves",
            detail = "Tiers are earned from outcomes, and you can cap them in Settings at any time.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The coach doesn't start with authority. It earns it. It begins by watching, and " +
                    "every level up is paid for with judged results on your own training."
            ),
            Heading("The five levels"),
            Bullets(
                listOf(
                    "Observing: it watches and says nothing",
                    "Proposing: it suggests, and you decide every change",
                    "Auto-applying: the kinds of change with a track record apply themselves, and all of them undo",
                    "Proactive: it plans blocks and starts projects, telling you first",
                    "Autonomous: it owns the program, acts first and tells you after"
                )
            ),
            Heading("How it moves"),
            Paragraph(
                "It moves up on a good run across enough judged calls, plus time. It moves down on " +
                    "a sustained bad run, or on you undoing its work again and again. Never on a " +
                    "single miss. A coach that dropped a level every time one call went wrong " +
                    "would spend its life bouncing up and down instead of coaching."
            ),
            Paragraph(
                "The top level is different. Getting there only earns the coach the right to ask. " +
                    "Full autonomy turns on when you say so, and not before."
            ),
            Callout("Trust the automation exactly as much as its record earns. The record is on the Coach page.")
        )
    )

    val takingDecisionsBack = Lesson(
        id = "coach.taking_decisions_back",
        track = LessonTrack.COACH,
        title = "How to take any decision back",
        summary = "Every move is watched, undoable, and cappable. The whole point is that you can stop using it.",
        unlock = LessonUnlock(
            label = "The first time the coach acts on its own",
            detail = "Only ever after you've raised its tier that far.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The coach changes things through the same screens you do. So anything it does, " +
                    "you can undo the same way you'd undo your own edit."
            ),
            Bullets(
                listOf(
                    "One change: undo it from the Coach page",
                    "A whole kind of change: cap the tier in Settings and it stops",
                    "Bigger changes past their undo window: the coach rebuilds the old shape and keeps every session you logged",
                    "Your own edits always win, and become rules the coach plans around"
                )
            ),
            Heading("Reading its record"),
            Paragraph(
                "Every change it applies gets a verdict once enough time has passed to tell: it " +
                    "worked, it didn't, or you weren't there for it. The third doesn't count " +
                    "against the coach, because a fortnight you spent ill or away says nothing " +
                    "about the advice."
            ),
            Callout(
                "The goal of this whole system is that you could do it yourself. Using the coach is " +
                    "a convenience, not a dependency."
            )
        )
    )

    val all: List<Lesson> = listOf(trustTiers, takingDecisionsBack)
}
