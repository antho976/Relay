package com.tally.app.ui.common

import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.MenuBook
import androidx.compose.material.icons.automirrored.rounded.ReceiptLong
import androidx.compose.material.icons.rounded.AccountBalance
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Bolt
import androidx.compose.material.icons.rounded.CardGiftcard
import androidx.compose.material.icons.rounded.ChildCare
import androidx.compose.material.icons.rounded.ConfirmationNumber
import androidx.compose.material.icons.rounded.CreditCard
import androidx.compose.material.icons.rounded.DirectionsBus
import androidx.compose.material.icons.rounded.DirectionsCar
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.FitnessCenter
import androidx.compose.material.icons.rounded.Flight
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.LocalCafe
import androidx.compose.material.icons.rounded.LocalGasStation
import androidx.compose.material.icons.rounded.MoreHoriz
import androidx.compose.material.icons.rounded.Payments
import androidx.compose.material.icons.rounded.Pets
import androidx.compose.material.icons.rounded.Replay
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.Restaurant
import androidx.compose.material.icons.rounded.Savings
import androidx.compose.material.icons.rounded.School
import androidx.compose.material.icons.rounded.ShoppingBag
import androidx.compose.material.icons.rounded.ShoppingCart
import androidx.compose.material.icons.rounded.Spa
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material.icons.rounded.Wifi
import androidx.compose.material.icons.rounded.Work
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
 * map to it.
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
    )

    fun of(key: String?): ImageVector = all[key] ?: Icons.Rounded.MoreHoriz

    val transfer: ImageVector get() = Icons.Rounded.SwapHoriz

    fun account(type: AccountType): ImageVector = when (type) {
        AccountType.CASH -> Icons.Rounded.Payments
        AccountType.CHEQUING -> Icons.Rounded.AccountBalance
        AccountType.SAVINGS -> Icons.Rounded.Savings
        AccountType.CREDIT -> Icons.Rounded.CreditCard
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
