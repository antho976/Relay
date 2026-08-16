package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * Library · Hypertrophy.
 *
 * Prose, not bullets. A Library article earns its length by explaining a mechanism and then saying
 * what to do differently, which a list of assertions cannot do. [LessonBlock.Bullets] stays
 * available for the rare genuinely enumerable thing, but more than one bullet block in an article
 * is a sign the paragraphs were never written.
 */
internal object LibraryHypertrophy {

    val proximityToFailure = Article(
        id = "library.proximity_to_failure",
        title = "How close to failure you need to go",
        deck = "Most of the growth is available a couple of reps short, and the last rep costs more than it returns",
        level = ArticleLevel.APPLIED,
        topics = listOf(ArticleTopic.HYPERTROPHY, ArticleTopic.PROGRAMMING),
        blocks = listOf(
            Paragraph(
                "Taking a set to the point where the next rep will not move is the clearest " +
                    "definition of training hard, which is why it became the default advice. It is " +
                    "also the most expensive rep in the set, and the evidence that it is required " +
                    "is thinner than the confidence with which it gets repeated."
            ),
            Heading("What the trials show"),
            Paragraph(
                "When total volume is matched, sets stopped a few reps short of failure grow " +
                    "muscle about as well as sets taken to failure. The relationship is not flat. " +
                    "Stop too far out, at five or six reps in reserve, and growth does fall away. " +
                    "But somewhere between zero and about three reps in reserve the differences " +
                    "between conditions are small and inconsistent from study to study, which is " +
                    "what a real but minor effect looks like when it is measured honestly."
            ),
            Paragraph(
                "Strength behaves differently from size, and in the opposite direction. Stopping " +
                    "short tends to favour strength, because a set taken to failure degrades the " +
                    "quality of every rep that follows it, in that session and across the days it " +
                    "takes to recover. If you care about the number on the bar, failure is a worse " +
                    "trade than it is for size."
            ),
            Paragraph(
                "There is a second reason this question keeps producing muddy results. Failure and " +
                    "volume trade against each other. Sets taken to failure force you to do fewer " +
                    "of them, and sets stopped short let you do more, so a study comparing the two " +
                    "is often comparing two different total doses wearing the same label."
            ),
            Heading("Why the last rep is expensive"),
            Paragraph(
                "Fatigue does not end when the set does. Reaching failure raises the recovery cost " +
                    "of a session out of proportion to the stimulus it adds, and that cost is paid " +
                    "by the sets after it, the sessions after those, and sometimes the rest of the " +
                    "week. You are trading a small amount of stimulus now for a measurable amount " +
                    "of capacity later."
            ),
            Paragraph(
                "This is also why the cost scales with the lift. A set of leg press to failure is " +
                    "recoverable. A set of squats to failure taxes your lower back, your ability to " +
                    "brace and your appetite for the next session, all for the same handful of " +
                    "extra fibres."
            ),
            Heading("What to do with it"),
            Paragraph(
                "Train most working sets with one to three reps left. Save true failure for the " +
                    "last set of an isolation exercise, where the recovery bill is small and the " +
                    "point of failure is easy to judge. On a heavy compound, failure buys you very " +
                    "little and costs you the most."
            ),
            Paragraph(
                "The harder problem is that most lifters are worse at estimating reps in reserve " +
                    "than they believe, and the error runs one way: sets you are sure are two short " +
                    "are often four. That is an argument for occasionally taking a set to genuine " +
                    "failure, not for the stimulus, but for the calibration. You cannot aim two " +
                    "reps short of a line you have never seen."
            ),
            Callout(
                "Go to failure to learn where it is, not to live there."
            )
        ),
        sources = listOf(
            Source(
                authors = "Refalo M, Helms E, Trexler E, Hamilton D, Fyfe J",
                title = "Influence of resistance training proximity-to-failure on skeletal muscle hypertrophy: a systematic review with meta-analysis",
                journal = "Sports Medicine",
                year = 2023
            ),
            Source(
                authors = "Grgic J, Schoenfeld B, Orazem J, Sabol F",
                title = "Effects of resistance training performed to repetition failure or non-failure on muscular strength and hypertrophy",
                journal = "Journal of Sport and Health Science",
                year = 2022
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
            )
        )
    )

    val howMuchVolume = Article(
        id = "library.how_much_volume",
        title = "How much volume you actually need",
        deck = "Around ten hard sets per muscle per week buys most of the growth, and the rest is paid for out of recovery",
        level = ArticleLevel.APPLIED,
        topics = listOf(ArticleTopic.HYPERTROPHY, ArticleTopic.PROGRAMMING),
        blocks = listOf(
            Paragraph(
                "Volume is the closest thing hypertrophy training has to a dial. More hard sets per " +
                    "muscle per week produce more growth, up to a point, and most of the argument " +
                    "is about where that point sits and how much the last few sets are worth."
            ),
            Heading("The shape of the curve"),
            Paragraph(
                "Across the meta-analyses the pattern is consistent: growth rises with weekly sets, " +
                    "and rises more slowly the further you go. Roughly ten hard sets per muscle per " +
                    "week is where most of the available growth has already happened. Going beyond " +
                    "that keeps adding, but each additional set returns less than the one before " +
                    "it, and somewhere past twenty the curve flattens far enough that it becomes " +
                    "hard to measure at all."
            ),
            Paragraph(
                "The honest reading is that the curve has a long shallow tail rather than a cliff. " +
                    "There is no set count at which growth switches off and no threshold you have " +
                    "to cross. Someone doing eight good sets a week is not failing, and someone " +
                    "doing twenty five is not necessarily doing better."
            ),
            Heading("What a set has to be to count"),
            Paragraph(
                "A set only counts if it was close enough to failure to matter, and if it actually " +
                    "loaded the muscle you are counting it for. Two sets of leg press taken near " +
                    "failure count for quadriceps. Six sets of the same exercise stopped well short " +
                    "largely do not, and neither do the incidental sets a muscle catches while " +
                    "acting as a stabiliser. Most inflated set counts come from here, which is why " +
                    "comparing your number against a study's number is less useful than it looks."
            ),
            Heading("Why more is not free"),
            Paragraph(
                "Volume is bought with recovery, and recovery is finite and shared. Sets added to " +
                    "one muscle are paid for out of the same account that funds every other muscle, " +
                    "your conditioning, your sleep debt and whatever is happening outside the gym. " +
                    "This is the mechanism behind the usual failure mode: a lifter adds volume " +
                    "everywhere at once, recovers from none of it, and concludes that more volume " +
                    "does not work."
            ),
            Paragraph(
                "It also explains why the useful ceiling moves. Your tolerance for volume is not a " +
                    "constant. It is a function of training age, how well you are eating and " +
                    "sleeping, and how much stress the rest of your life is producing this month. A " +
                    "number that worked in a good block will be too much in a bad one."
            ),
            Heading("What to do with it"),
            Paragraph(
                "Start nearer ten hard sets per muscle per week than twenty, hold it long enough to " +
                    "see whether it is working, and add only where progress has actually stalled. " +
                    "Add to one or two muscles at a time so you can tell what caused the change. If " +
                    "performance falls across several exercises at once rather than one, you have " +
                    "found your ceiling rather than a plateau."
            ),
            Callout(
                "Everything past ten hard sets is real, small, and paid for out of recovery you also need elsewhere."
            )
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

    val all: List<Article> = listOf(proximityToFailure, howMuchVolume)
}
