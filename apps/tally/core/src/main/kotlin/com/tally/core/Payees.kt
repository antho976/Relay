package com.tally.core

/**
 * What a bank's description becomes in the ledger, and the category it most likely belongs to.
 * Banks write "ACHAT - METRO PLUS #123 MONTREAL QC"; the ledger wants "Metro Plus Montreal QC"
 * filed under Groceries. All of it is a guess the owner can change, and the import says how many
 * lines it filed itself.
 */
object Payees {

    /** The channel words banks put before the payee, in normalized form, longest first where they overlap. */
    private val PREFIXES = listOf(
        "paiement facture accesd internet", "paiement facture accesd mobile", "paiement facture accesd",
        "paiement de facture", "paiement facture", "paiement direct", "achat interac", "achat",
        "accesd internet", "accesd mobile", "accesd",
        "interac purchase", "visa debit purchase", "debit card purchase", "pos purchase", "point of sale", "purchase", "pos",
        "online bill payment", "bill payment", "pre authorized debit", "preauthorized debit",
        "prelevement automatique", "prelevement", "depot direct", "direct deposit from", "direct deposit",
        "retrait au ga", "retrait",
    )

    /** Words kept in capitals when a shouted description is set in title case. */
    private val ACRONYMS = setOf(
        "IGA", "SAQ", "SAAQ", "STM", "RTC", "STS", "RTL", "TTC", "SQDC", "IKEA", "KFC", "DQ", "PC", "TD", "RBC", "BMO",
        "CIBC", "HSBC", "UPS", "ATM", "USA", "CA", "QC", "ON", "BC", "AB", "MB", "SK", "NB", "NS", "PE", "NL", "YT", "NT",
        "NU", "TFSA", "RRSP", "FHSA", "CELI", "REER", "GST", "TPS", "TVQ", "H&M", "A&W", "TV", "MEC", "SAIL", "CO",
    )

    private val REFERENCE = Regex("""(?:[#*xX]{0,4}\d{4,}|#\s?\d+)""")

    /**
     * The payee as the ledger shows it: the channel words dropped, card and reference numbers
     * dropped, and a description in all capitals set in title case. Never empty when [raw] is not.
     */
    fun clean(raw: String): String {
        var text = raw.replace(Regex("\\s+"), " ").trim()
        if (text.isEmpty()) return text
        var changed = true
        while (changed) {
            changed = false
            val norm = BankStatements.normalize(text)
            val prefix = PREFIXES.firstOrNull { norm == it || norm.startsWith("$it ") } ?: break
            val rest = dropWords(text, prefix.split(' ').size).trimStart(' ', '-', '/', ':', '.')
            if (rest.isNotBlank()) {
                text = rest
                changed = true
            }
        }
        text = text.replace(REFERENCE, " ").replace(Regex("\\s+"), " ").trim().trim('-', '/', ':', '.', ' ')
        if (text.isEmpty()) return raw.trim()
        val letters = text.filter { it.isLetter() }
        return if (letters.isNotEmpty() && letters.count { it.isUpperCase() } >= letters.length * 0.8) titleCase(text) else text
    }

    /** [text] with its first [n] words gone, where a word ends at a space, a dash or a slash. */
    private fun dropWords(text: String, n: Int): String {
        var i = 0
        var words = 0
        while (i < text.length && words < n) {
            while (i < text.length && !text[i].isLetterOrDigit()) i++
            while (i < text.length && text[i].isLetterOrDigit()) i++
            words++
        }
        return text.substring(i)
    }

    private fun titleCase(text: String): String = text.split(' ').joinToString(" ") { word ->
        when {
            word.uppercase() in ACRONYMS -> word.uppercase()
            word.any { it.isDigit() } -> word
            else -> word.lowercase().split('-').joinToString("-") { part ->
                part.replaceFirstChar { it.titlecase() }
            }
        }
    }

    /**
     * The words that name a payee for learning from the ledger: "Metro Plus Montreal QC" and
     * "METRO PLUS #4411" both key as "metro plus". Empty when nothing is left.
     */
    fun key(note: String): String =
        BankStatements.normalize(note).replace(Regex("\\d+"), " ").split(' ').filter { it.length > 1 }.take(2).joinToString(" ")

    /** Moves between the owner's own accounts: card payments, transfers to savings. Never an Interac e-Transfer to a person. */
    private val TRANSFER_WORDS = listOf(
        "virement", "transfer", "transfert", "paiement carte", "paiement visa", "paiement mastercard", "paiement de carte",
        "carte de credit", "credit card payment", "card payment", "payment thank you", "payment received", "paiement recu",
        "merci pour votre paiement", "paiement merci", "contribution", "cotisation",
    )
    private val TRANSFER_CODES = setOf("TRFIN", "TRFOUT", "TRFINTF", "TRF_IN", "TRF_OUT", "TRANSFER", "INTERNAL_TRANSFER")

    fun looksLikeTransfer(description: String, code: String? = null): Boolean {
        if (code != null && code.uppercase() in TRANSFER_CODES) return true
        val n = " " + BankStatements.normalize(description) + " "
        if ("interac" in n || " e transfer" in n || " etransfer" in n) return false
        return TRANSFER_WORDS.any { " $it" in n }
    }

    /**
     * The default category a description most likely belongs to, by name ("Groceries"), or null.
     * Keys are matched at the start of a word; one ending in a space must be the whole word, so
     * "metro " never files a "Metropolis" ticket under Groceries. Order matters: subscriptions
     * before shopping (Amazon Prime), dining before transport (Uber Eats).
     */
    fun guessCategory(description: String, income: Boolean, code: String? = null): String? {
        val n = " " + BankStatements.normalize(description) + " "
        if (income) {
            if (code != null && code.uppercase() == "INT") return "Other income"
            return INCOME_RULES.firstOrNull { (_, keys) -> keys.any { " $it" in n } }?.first
        }
        return SPENDING_RULES.firstOrNull { (_, keys) -> keys.any { " $it" in n } }?.first
    }

    private val SPENDING_RULES: List<Pair<String, List<String>>> = listOf(
        "Subscriptions" to listOf(
            "netflix", "spotify", "disney plus", "disneyplus", "crave", "prime video", "primevideo", "amazon prime",
            "apple com bill", "apple music", "icloud", "google one", "google storage", "youtube premium", "youtubepremium",
            "patreon", "adobe", "microsoft 365", "dropbox", "chatgpt", "openai", "anthropic", "audible", "paramount",
            "club illico", "illico", "tou tv", "deezer", "playstation plus", "xbox game pass",
        ),
        "Dining" to listOf(
            "restaurant", "resto ", "mcdonald", "tim hortons", "tims ", "starbucks", "second cup", "subway ", "pizza", "sushi",
            "uber eats", "ubereats", "doordash", "skipthedishes", "skip the dishes", "cafe ", "bistro", "brasserie",
            "poulet rouge", "st hubert", "benny", "la cage", "pret a manger", "domino", "burger", "bagel", "shawarma",
            "poke", "pho ", "thai ", "kfc ", "popeyes", "harvey", "mary brown", "dairy queen", "dq ", "boston pizza",
            "cora ", "chocolato", "poutine", "valentine", "booster juice", "freshii", "chipotle", "five guys",
            "la belle province", "dic ann", "a w ", "wendy", "taco",
        ),
        "Groceries" to listOf(
            "metro ", "iga ", "provigo", "maxi ", "super c ", "loblaws", "sobeys", "adonis", "marche ", "epicerie",
            "fruiterie", "boucherie", "poissonnerie", "boulangerie", "no frills", "food basics", "freshco", "foodland",
            "voila ", "avril ", "rachelle bery", "intermarche", "costco", "mayrand", "kim phat", "whole foods",
            "farm boy", "longo", "save on foods", "safeway", "superstore", "t t supermarket", "segal",
        ),
        "Transport" to listOf(
            "stm ", "opus", "exo ", "rtc ", "sts ", "rtl ", "ttc ", "translink", "presto", "uber ", "lyft", "taxi",
            "communauto", "bixi", "via rail", "viarail", "petro canada", "petrocan", "shell ", "esso ", "ultramar",
            "couche tard", "irving", "pioneer", "husky", "sunoco", "parking", "stationnement", "indigo park", "saaq ",
            "car wash", "lave auto", "amigo express", "orleans express", "mobilite",
        ),
        "Housing" to listOf(
            "loyer", "rent ", "hypotheque", "mortgage", "frais de condo", "condo fees", "assurance habitation",
            "home insurance", "taxes municipales", "property tax",
        ),
        "Utilities" to listOf("hydro quebec", "hydro ", "energir", "gaz metro", "enbridge", "fortis", "epcor", "bc hydro", "water "),
        "Phone & internet" to listOf(
            "videotron", "bell ", "bell canada", "rogers", "fido", "telus", "koodo", "fizz", "virgin plus", "virgin mobile",
            "public mobile", "freedom mobile", "cogeco", "ebox", "oxio", "distributel", "teksavvy", "chatr", "lucky mobile",
        ),
        "Health" to listOf(
            "pharmaprix", "jean coutu", "uniprix", "familiprix", "shoppers drug", "proxim", "brunet", "pharmacie", "pharmacy",
            "clinique", "clinic", "dentist", "dentaire", "dental", "optometr", "lunetterie", "physio", "massotherap", "chiropr",
            "energie cardio", "econofitness", "nautilus plus", "goodlife", "gym ", "yoga", "crossfit", "psycholog",
        ),
        "Entertainment" to listOf(
            "cineplex", "cinema", "steam ", "steampowered", "playstation", "xbox", "nintendo", "ticketmaster", "spectra",
            "evenko", "bar ", "pub ", "billard", "bowling", "quilles", "musee", "museum", "theatre", "concert",
            "loto quebec", "festival",
        ),
        "Travel" to listOf(
            "air canada", "westjet", "porter air", "air transat", "sunwing", "flair air", "expedia", "booking com",
            "airbnb", "hotel", "marriott", "hilton", "hyatt", "best western", "holiday inn", "aeroport", "airport",
        ),
        "Shopping" to listOf(
            "amazon", "amzn", "walmart", "canadian tire", "winners", "simons", "la baie", "hudson s bay", "best buy",
            "ikea", "dollarama", "home depot", "rona ", "reno depot", "structube", "apple store", "aliexpress", "shein",
            "etsy", "ebay", "sports experts", "sport expert", "decathlon", "uniqlo", "h m ", "zara", "old navy", "gap ",
            "indigo", "renaud bray", "archambault", "bureau en gros", "staples", "jysk", "giant tiger", "ardene", "aldo",
            "la vie en rose", "atmosphere", "value village", "village des valeurs", "dollar tree", "michaels", "lego",
        ),
    )

    private val INCOME_RULES: List<Pair<String, List<String>>> = listOf(
        "Refunds" to listOf("refund", "remboursement", "retour", "return", "reversal", "annulation"),
        "Salary" to listOf("paie ", "payroll", "salaire", "pay ", "depot de paie", "employeur"),
        "Other income" to listOf(
            "interet", "interest", "dividend", "dividende", "cashback", "remise", "gouv", "gouvernement", "canada fed",
            "gst ", "tps ", "ccb ", "allocation",
        ),
    )
}
