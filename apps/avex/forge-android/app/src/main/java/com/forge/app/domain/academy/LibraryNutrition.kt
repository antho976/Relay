package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/** Library · Nutrition. */
internal object LibraryNutrition {

    val proteinIntake = Article(
        id = "library.protein_intake",
        title = "How much protein you need",
        deck = "About 1.6 grams per kilo a day, more when dieting, and almost everything else is detail",
        level = ArticleLevel.APPLIED,
        topics = listOf(ArticleTopic.NUTRITION),
        blocks = listOf(
            Paragraph(
                "Protein is the most over-discussed and least complicated part of nutrition for " +
                    "lifters. The dose that matters is a daily one, the number is lower than the " +
                    "supplement industry implies and higher than the general population guidelines " +
                    "say, and almost everything else is detail."
            ),
            Heading("The number"),
            Paragraph(
                "The best available estimate for maximising muscle gained from resistance training " +
                    "is about 1.6 grams per kilo of bodyweight per day. Above that the measured " +
                    "benefit becomes hard to detect. The confidence interval in the largest " +
                    "meta-analysis stretches to around 2.2 grams per kilo, which is why the higher " +
                    "figure gets quoted, but the average effect past 1.6 is close to nothing."
            ),
            Paragraph(
                "Two situations justify going higher. Dieting is one: in a deficit protein does " +
                    "more work defending the muscle you already have, and intakes toward 2.3 to 3.1 " +
                    "grams per kilo of lean mass have support in lean, trained athletes. The other " +
                    "is age, where the same dose produces a smaller response and more is needed to " +
                    "get the same effect."
            ),
            Heading("Distribution matters less than you were told"),
            Paragraph(
                "The idea that the body can only use twenty or thirty grams at a sitting comes from " +
                    "studies measuring muscle protein synthesis over a few hours, which is not the " +
                    "same as measuring muscle. Over a day, the total is what predicts the outcome. " +
                    "Spreading intake across three or four meals is a reasonable default because it " +
                    "makes the total easier to hit, not because a larger meal is wasted."
            ),
            Heading("Source"),
            Paragraph(
                "Animal protein has an advantage per gram, mostly because it carries more leucine " +
                    "and is more completely digested. The advantage is real and small, and it " +
                    "disappears if a plant-based diet includes more total protein and a reasonable " +
                    "variety. There is no source you have to eat."
            ),
            Heading("What to do with it"),
            Paragraph(
                "Take your bodyweight in kilos, multiply by 1.6, and treat the result as a floor " +
                    "rather than a target to chase. If you are cutting, aim nearer 2.0 and expect " +
                    "to feel fuller for it. If hitting the number reliably needs a supplement, use " +
                    "one, and understand that you are buying convenience rather than an effect " +
                    "powder has and food does not."
            ),
            Callout(
                "Hit the daily total. Everything past that is optimisation of something already handled."
            )
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

    val all: List<Article> = listOf(proteinIntake)
}
