package com.tally.app.ui.common

import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.MenuBook
import androidx.compose.material.icons.automirrored.rounded.ReceiptLong
import androidx.compose.material.icons.automirrored.rounded.ShowChart
import androidx.compose.material.icons.automirrored.rounded.TrendingUp
import androidx.compose.material.icons.rounded.AccountBalance
import androidx.compose.material.icons.rounded.AccountBalanceWallet
import androidx.compose.material.icons.rounded.Apartment
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.BakeryDining
import androidx.compose.material.icons.rounded.BeachAccess
import androidx.compose.material.icons.rounded.Bolt
import androidx.compose.material.icons.rounded.Build
import androidx.compose.material.icons.rounded.Business
import androidx.compose.material.icons.rounded.Cake
import androidx.compose.material.icons.rounded.CameraAlt
import androidx.compose.material.icons.rounded.CarRepair
import androidx.compose.material.icons.rounded.CardGiftcard
import androidx.compose.material.icons.rounded.Casino
import androidx.compose.material.icons.rounded.Celebration
import androidx.compose.material.icons.rounded.Chair
import androidx.compose.material.icons.rounded.Checkroom
import androidx.compose.material.icons.rounded.ChildCare
import androidx.compose.material.icons.rounded.ChildFriendly
import androidx.compose.material.icons.rounded.Church
import androidx.compose.material.icons.rounded.CleaningServices
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.ConfirmationNumber
import androidx.compose.material.icons.rounded.ContentCut
import androidx.compose.material.icons.rounded.CreditCard
import androidx.compose.material.icons.rounded.CurrencyBitcoin
import androidx.compose.material.icons.rounded.Diamond
import androidx.compose.material.icons.rounded.DirectionsBoat
import androidx.compose.material.icons.rounded.DirectionsBus
import androidx.compose.material.icons.rounded.DirectionsCar
import androidx.compose.material.icons.rounded.ElectricCar
import androidx.compose.material.icons.rounded.Elderly
import androidx.compose.material.icons.rounded.EmojiFoodBeverage
import androidx.compose.material.icons.rounded.FamilyRestroom
import androidx.compose.material.icons.rounded.Fastfood
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.FitnessCenter
import androidx.compose.material.icons.rounded.Flight
import androidx.compose.material.icons.rounded.Gavel
import androidx.compose.material.icons.rounded.Handshake
import androidx.compose.material.icons.rounded.Headphones
import androidx.compose.material.icons.rounded.Hiking
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.Hotel
import androidx.compose.material.icons.rounded.Icecream
import androidx.compose.material.icons.rounded.Kitchen
import androidx.compose.material.icons.rounded.Lightbulb
import androidx.compose.material.icons.rounded.Liquor
import androidx.compose.material.icons.rounded.LocalAtm
import androidx.compose.material.icons.rounded.LocalBar
import androidx.compose.material.icons.rounded.LocalCafe
import androidx.compose.material.icons.rounded.LocalFireDepartment
import androidx.compose.material.icons.rounded.LocalFlorist
import androidx.compose.material.icons.rounded.LocalGasStation
import androidx.compose.material.icons.rounded.LocalHospital
import androidx.compose.material.icons.rounded.LocalLaundryService
import androidx.compose.material.icons.rounded.LocalParking
import androidx.compose.material.icons.rounded.LocalPharmacy
import androidx.compose.material.icons.rounded.LocalPizza
import androidx.compose.material.icons.rounded.LocalShipping
import androidx.compose.material.icons.rounded.LocalTaxi
import androidx.compose.material.icons.rounded.Luggage
import androidx.compose.material.icons.rounded.LunchDining
import androidx.compose.material.icons.rounded.Mail
import androidx.compose.material.icons.rounded.MedicalServices
import androidx.compose.material.icons.rounded.Medication
import androidx.compose.material.icons.rounded.MoreHoriz
import androidx.compose.material.icons.rounded.Movie
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.Park
import androidx.compose.material.icons.rounded.Payments
import androidx.compose.material.icons.rounded.PedalBike
import androidx.compose.material.icons.rounded.Pets
import androidx.compose.material.icons.rounded.Pool
import androidx.compose.material.icons.rounded.Psychology
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.RamenDining
import androidx.compose.material.icons.rounded.Replay
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RequestQuote
import androidx.compose.material.icons.rounded.Restaurant
import androidx.compose.material.icons.rounded.Savings
import androidx.compose.material.icons.rounded.School
import androidx.compose.material.icons.rounded.SelfImprovement
import androidx.compose.material.icons.rounded.Sell
import androidx.compose.material.icons.rounded.Shield
import androidx.compose.material.icons.rounded.ShoppingBag
import androidx.compose.material.icons.rounded.ShoppingCart
import androidx.compose.material.icons.rounded.Smartphone
import androidx.compose.material.icons.rounded.Spa
import androidx.compose.material.icons.rounded.SportsBar
import androidx.compose.material.icons.rounded.SportsEsports
import androidx.compose.material.icons.rounded.SportsSoccer
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material.icons.rounded.Storefront
import androidx.compose.material.icons.rounded.Subway
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material.icons.rounded.TheaterComedy
import androidx.compose.material.icons.rounded.Toys
import androidx.compose.material.icons.rounded.Train
import androidx.compose.material.icons.rounded.Tv
import androidx.compose.material.icons.rounded.TwoWheeler
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VolunteerActivism
import androidx.compose.material.icons.rounded.Watch
import androidx.compose.material.icons.rounded.WaterDrop
import androidx.compose.material.icons.rounded.Wifi
import androidx.compose.material.icons.rounded.WineBar
import androidx.compose.material.icons.rounded.Work
import androidx.compose.material.icons.rounded.Yard
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.theme.categoryColor
import com.tally.core.AccountType

/**
 * The drawn glyph for every category icon key. One library (Material Rounded), one stroke, so the
 * family reads as one set. [com.tally.core.Defaults.iconKeys] is the contract; a test holds this
 * map to it, and [groups] to this map.
 */
object CategoryIcons {
    val all: Map<String, ImageVector> = linkedMapOf(
        "cart" to Icons.Rounded.ShoppingCart,
        "dining" to Icons.Rounded.Restaurant,
        "coffee" to Icons.Rounded.LocalCafe,
        "transport" to Icons.Rounded.DirectionsBus,
        "car" to Icons.Rounded.DirectionsCar,
        "fuel" to Icons.Rounded.LocalGasStation,
        "home" to Icons.Rounded.Home,
        "bolt" to Icons.Rounded.Bolt,
        "wifi" to Icons.Rounded.Wifi,
        "bag" to Icons.Rounded.ShoppingBag,
        "health" to Icons.Rounded.Favorite,
        "ticket" to Icons.Rounded.ConfirmationNumber,
        "repeat" to Icons.Rounded.Repeat,
        "flight" to Icons.Rounded.Flight,
        "gift" to Icons.Rounded.CardGiftcard,
        "dots" to Icons.Rounded.MoreHoriz,
        "work" to Icons.Rounded.Work,
        "spark" to Icons.Rounded.AutoAwesome,
        "refund" to Icons.Rounded.Replay,
        "pets" to Icons.Rounded.Pets,
        "school" to Icons.Rounded.School,
        "child" to Icons.Rounded.ChildCare,
        "sport" to Icons.Rounded.FitnessCenter,
        "beauty" to Icons.Rounded.Spa,
        "savings" to Icons.Rounded.Savings,
        "bank" to Icons.Rounded.AccountBalance,
        "card" to Icons.Rounded.CreditCard,
        "cash" to Icons.Rounded.Payments,
        "fees" to Icons.AutoMirrored.Rounded.ReceiptLong,
        "book" to Icons.AutoMirrored.Rounded.MenuBook,
        // The grouped picker's additions (2026-10-05). Appended: the keys are stored.
        "pizza" to Icons.Rounded.LocalPizza,
        "fastfood" to Icons.Rounded.Fastfood,
        "lunch" to Icons.Rounded.LunchDining,
        "ramen" to Icons.Rounded.RamenDining,
        "bakery" to Icons.Rounded.BakeryDining,
        "icecream" to Icons.Rounded.Icecream,
        "bar" to Icons.Rounded.LocalBar,
        "beer" to Icons.Rounded.SportsBar,
        "wine" to Icons.Rounded.WineBar,
        "liquor" to Icons.Rounded.Liquor,
        "tea" to Icons.Rounded.EmojiFoodBeverage,
        "kitchen" to Icons.Rounded.Kitchen,
        "train" to Icons.Rounded.Train,
        "subway" to Icons.Rounded.Subway,
        "taxi" to Icons.Rounded.LocalTaxi,
        "bike" to Icons.Rounded.PedalBike,
        "scooter" to Icons.Rounded.TwoWheeler,
        "ev" to Icons.Rounded.ElectricCar,
        "parking" to Icons.Rounded.LocalParking,
        "carrepair" to Icons.Rounded.CarRepair,
        "boat" to Icons.Rounded.DirectionsBoat,
        "apartment" to Icons.Rounded.Apartment,
        "water" to Icons.Rounded.WaterDrop,
        "heat" to Icons.Rounded.LocalFireDepartment,
        "furniture" to Icons.Rounded.Chair,
        "repairs" to Icons.Rounded.Build,
        "cleaning" to Icons.Rounded.CleaningServices,
        "laundry" to Icons.Rounded.LocalLaundryService,
        "garden" to Icons.Rounded.Yard,
        "insurance" to Icons.Rounded.Shield,
        "phone" to Icons.Rounded.Smartphone,
        "tv" to Icons.Rounded.Tv,
        "clothes" to Icons.Rounded.Checkroom,
        "computer" to Icons.Rounded.Computer,
        "headphones" to Icons.Rounded.Headphones,
        "toys" to Icons.Rounded.Toys,
        "flowers" to Icons.Rounded.LocalFlorist,
        "jewelry" to Icons.Rounded.Diamond,
        "store" to Icons.Rounded.Storefront,
        "watch" to Icons.Rounded.Watch,
        "camera" to Icons.Rounded.CameraAlt,
        "hospital" to Icons.Rounded.LocalHospital,
        "medication" to Icons.Rounded.Medication,
        "pharmacy" to Icons.Rounded.LocalPharmacy,
        "medical" to Icons.Rounded.MedicalServices,
        "haircut" to Icons.Rounded.ContentCut,
        "wellness" to Icons.Rounded.SelfImprovement,
        "therapy" to Icons.Rounded.Psychology,
        "eyes" to Icons.Rounded.Visibility,
        "movie" to Icons.Rounded.Movie,
        "games" to Icons.Rounded.SportsEsports,
        "music" to Icons.Rounded.MusicNote,
        "party" to Icons.Rounded.Celebration,
        "outdoors" to Icons.Rounded.Park,
        "soccer" to Icons.Rounded.SportsSoccer,
        "hiking" to Icons.Rounded.Hiking,
        "pool" to Icons.Rounded.Pool,
        "casino" to Icons.Rounded.Casino,
        "theatre" to Icons.Rounded.TheaterComedy,
        "art" to Icons.Rounded.Palette,
        "hotel" to Icons.Rounded.Hotel,
        "luggage" to Icons.Rounded.Luggage,
        "beach" to Icons.Rounded.BeachAccess,
        "baby" to Icons.Rounded.ChildFriendly,
        "family" to Icons.Rounded.FamilyRestroom,
        "elderly" to Icons.Rounded.Elderly,
        "charity" to Icons.Rounded.VolunteerActivism,
        "church" to Icons.Rounded.Church,
        "birthday" to Icons.Rounded.Cake,
        "invest" to Icons.AutoMirrored.Rounded.TrendingUp,
        "crypto" to Icons.Rounded.CurrencyBitcoin,
        "tax" to Icons.Rounded.RequestQuote,
        "wallet" to Icons.Rounded.AccountBalanceWallet,
        "loan" to Icons.Rounded.Handshake,
        "legal" to Icons.Rounded.Gavel,
        "business" to Icons.Rounded.Business,
        "atm" to Icons.Rounded.LocalAtm,
        "sale" to Icons.Rounded.Sell,
        "star" to Icons.Rounded.Star,
        "idea" to Icons.Rounded.Lightbulb,
        "online" to Icons.Rounded.Public,
        "cloud" to Icons.Rounded.Cloud,
        "delivery" to Icons.Rounded.LocalShipping,
        "mail" to Icons.Rounded.Mail,
    )

    /**
     * The picker's sections, in reading order. Every key of [all] sits in exactly one; a test
     * holds that, so a new glyph cannot be drawn and never offered.
     */
    val groups: List<Pair<String, List<String>>> = listOf(
        "Food and drink" to listOf(
            "cart", "dining", "coffee", "tea", "pizza", "fastfood", "lunch", "ramen", "bakery", "icecream", "kitchen",
            "bar", "beer", "wine", "liquor",
        ),
        "Getting around" to listOf("transport", "train", "subway", "car", "taxi", "fuel", "ev", "parking", "carrepair", "bike", "scooter", "boat", "flight"),
        "Home and bills" to listOf(
            "home", "apartment", "bolt", "water", "heat", "wifi", "phone", "tv", "insurance", "furniture", "repairs", "cleaning",
            "laundry", "garden", "repeat",
        ),
        "Shopping" to listOf("bag", "store", "clothes", "computer", "headphones", "camera", "watch", "jewelry", "toys", "flowers", "book", "gift"),
        "Health and care" to listOf("health", "medical", "hospital", "pharmacy", "medication", "eyes", "therapy", "wellness", "sport", "beauty", "haircut"),
        "Going out" to listOf(
            "ticket", "movie", "music", "theatre", "party", "games", "casino", "art", "outdoors", "hiking", "soccer", "pool",
            "hotel", "luggage", "beach",
        ),
        "People" to listOf("child", "baby", "school", "family", "elderly", "pets", "charity", "church", "birthday"),
        "Money" to listOf(
            "work", "business", "spark", "refund", "sale", "savings", "invest", "crypto", "bank", "card", "cash", "wallet", "atm",
            "fees", "tax", "loan", "legal",
        ),
        "Other" to listOf("dots", "star", "idea", "online", "cloud", "delivery", "mail"),
    )

    fun of(key: String?): ImageVector = all[key] ?: Icons.Rounded.MoreHoriz

    val transfer: ImageVector get() = Icons.Rounded.SwapHoriz

    fun account(type: AccountType): ImageVector = when (type) {
        AccountType.CASH -> Icons.Rounded.Payments
        AccountType.CHEQUING -> Icons.Rounded.AccountBalance
        AccountType.SAVINGS -> Icons.Rounded.Savings
        AccountType.CREDIT -> Icons.Rounded.CreditCard
        AccountType.INVESTMENT -> Icons.AutoMirrored.Rounded.ShowChart
    }
}

/**
 * A category's badge: its glyph in its own hue on a 15% wash of that hue. The colour is a data
 * series (which category), so it is spent here and in charts, never on text.
 */
@Composable
fun CategoryBadge(icon: String?, color: Int?, modifier: Modifier = Modifier, size: Dp = 44.dp) {
    val hue = categoryColor(color)
    GlyphBadge(CategoryIcons.of(icon), modifier, tint = hue, fill = hue.copy(alpha = 0.15f), size = size)
}

/** A transfer has no category; it gets the neutral swap glyph. */
@Composable
fun TransferBadge(modifier: Modifier = Modifier, size: Dp = 44.dp) {
    GlyphBadge(CategoryIcons.transfer, modifier, tint = MaterialTheme.colorScheme.onSurfaceVariant, size = size)
}

@Composable
fun AccountBadge(type: AccountType, modifier: Modifier = Modifier, size: Dp = 44.dp) {
    GlyphBadge(CategoryIcons.account(type), modifier.size(size), tint = MaterialTheme.colorScheme.onBackground, size = size)
}
