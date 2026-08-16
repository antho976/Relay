package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/** Library · Recovery. */
internal object LibraryRecovery {

    val sleepAndTraining = Article(
        id = "library.sleep_and_training",
        title = "What sleep actually does for training",
        deck = "One bad night changes how training feels more than what it does. A bad month changes what it does",
        level = ArticleLevel.BASICS,
        topics = listOf(ArticleTopic.RECOVERY),
        blocks = listOf(
            Paragraph(
                "Sleep is the only recovery intervention with a large effect and no cost, which is " +
                    "an unusual combination and the reason it gets recommended so relentlessly. It " +
                    "is also the one most people cannot fix by wanting to, so it is worth knowing " +
                    "what actually degrades and by how much."
            ),
            Heading("What short sleep costs you"),
            Paragraph(
                "One bad night is not a training emergency. Acute sleep loss has a modest effect on " +
                    "maximal strength and a much larger one on everything that depends on sustained " +
                    "effort or judgement: work capacity across a session, reaction time, and your " +
                    "own rating of how hard a given weight felt. The bar does not get heavier. It " +
                    "feels heavier, and you do fewer good sets before you stop."
            ),
            Paragraph(
                "Accumulated short sleep is a different problem. Sustained restriction changes the " +
                    "balance of what you gain and lose rather than just how you feel. In a " +
                    "controlled trial where dieters slept about five and a half hours instead of " +
                    "eight and a half, total weight lost was similar, but the share of it coming " +
                    "from lean mass rose sharply. Same deficit, worse outcome."
            ),
            Heading("Why"),
            Paragraph(
                "The mechanisms are not fully settled, and anyone claiming otherwise is selling " +
                    "something. What is reasonably established is that restricted sleep raises " +
                    "evening cortisol, blunts insulin sensitivity, and shifts the appetite hormones " +
                    "in the direction of eating more. The muscle protein synthesis story is more " +
                    "speculative than it is usually presented. It is fair to say the outcome data " +
                    "are stronger than the explanation for them."
            ),
            Heading("What is worth doing"),
            Paragraph(
                "Regularity matters more than duration for most people, because a consistent " +
                    "schedule is achievable and an extra hour usually is not. Going to bed and " +
                    "getting up inside roughly the same half hour window most days does more than " +
                    "any supplement marketed for the purpose."
            ),
            Paragraph(
                "If you genuinely cannot get the hours, train anyway, but train to the day you are " +
                    "having rather than the one you planned. Keep the load, cut a set or two, and " +
                    "drop the sets you were doing for volume rather than for progress. A short " +
                    "session honestly logged is worth more, to you and to any coach reading it, " +
                    "than a full one you had to abandon halfway."
            ),
            Callout(
                "Protect the schedule before you chase the hours. Consistency is the part you control."
            )
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
            )
        )
    )

    val all: List<Article> = listOf(sleepAndTraining)
}
