package com.quietsoftware.relay.ui.threads

import com.quietsoftware.relay.data.Relay as RelayData
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** A chart a reply wrote its own numbers for: decimals or decimal strings, a unit, a projection. */
class ChartsTest {
    private fun data(json: String) = Json.parseToJsonElement(json).jsonObject

    private val cad = Money("CAD", 2)

    @Test
    fun amountsAreDecimalsInTheCurrencysDigits() {
        val n = readWritten(data("""{"labels": ["Nov", "Dec", "Jan"], "series": [{"name": "Savings", "values": [1234.56, "1300.5", 1400]}]}"""), cad)
        assertEquals(listOf(123456L, 130050L, 140000L), n.series.single().values)
        assertTrue(n.fmt is Fmt.Cash)
        val yen = readWritten(data("""{"labels": ["Nov"], "series": [{"name": "Savings", "values": [" 1500 "]}]}"""), Money("JPY", 0))
        assertEquals(listOf(1500L), yen.series.single().values)
    }

    @Test
    fun aMissingOrGarbagePointIsLeftOutNotDrawnAsZero() {
        val n = readWritten(data("""{"labels": ["a", "b", "c", "d", "e", "f"], "series": [{"name": "x", "values": [10, null, "abc", true, 0]}]}"""), cad)
        assertEquals(listOf(1000L, null, null, null, 0L, null), n.series.single().values)
    }

    @Test(timeout = 2_000)
    fun anExponentTooLargeOrTooSmallIsAnsweredWithoutWorkingItOut() {
        assertNull(decimal(JsonPrimitive("1e100000000"), 2))
        assertNull(decimal(JsonPrimitive("-1E100000000"), 2))
        assertEquals(0L, decimal(JsonPrimitive("1e-100000000"), 2))
        assertEquals(0L, decimal(JsonPrimitive("0.004"), 2))
        assertEquals(1L, decimal(JsonPrimitive("0.006"), 2))
        assertEquals(92_233_720_368_547_758L, decimal(JsonPrimitive("922337203685477.58"), 2))
        assertNull(decimal(JsonPrimitive("92233720368547758.08"), 2))
        assertEquals(150L, decimal(JsonPrimitive("1.5e0"), 2))
    }

    @Test
    fun aUnitCountsInTenthsOfIt() {
        val kg = readWritten(data("""{"labels": ["Mon", "Tue"], "series": [{"name": "Weight", "values": [80.2, "81"]}], "unit": "kg"}"""), cad)
        assertEquals(listOf(802L, 810L), kg.series.single().values)
        assertEquals("80.2 kg", kg.fmt.format(802))
        assertEquals("81 kg", kg.fmt.format(810))
        val rate = readWritten(data("""{"labels": ["Q1"], "series": [{"name": "Rate", "values": [12.5]}], "unit": "%"}"""), cad)
        assertEquals("12.5%", rate.fmt.format(125))
        val trips = readWritten(data("""{"labels": ["Q1"], "series": [{"name": "Trips", "values": [3]}], "unit": ""}"""), cad)
        assertEquals(listOf(30L), trips.series.single().values)
        assertEquals("3", trips.fmt.whole(30))
    }

    @Test
    fun aProjectedSeriesSaysSo() {
        val n = readWritten(data("""{"labels": ["Oct", "Nov"], "series": [{"name": "Balance", "values": [100, null]}, {"name": "Balance", "values": [null, "150"], "projected": true}]}"""), cad)
        assertFalse(n.series[0].projected)
        assertTrue(n.series[1].projected)
        assertEquals(listOf(null, 15000L), n.series[1].values)
        assertEquals("Balance", n.series[0].title())
        assertEquals("Balance (projected)", n.series[1].title())
        assertEquals("Total (projected)", Series("", emptyList(), projected = true).title())
    }

    @Test
    fun aPriceLineIsOneSeriesOfClosesWithItsTradesAtTheirBar() {
        val n = readPrice(data("""{"product": "BTC-USD", "candles": [{"start": 1760000000, "close": 61234.5}, {"start": 1760003600, "close": "61300"}], "markers": [{"at": 1760003700, "price": 61290.25, "side": "buy"}]}"""))
        val line = n.series.single()
        assertEquals("BTC-USD", line.name)
        assertEquals(listOf(6123450L, 6130000L), line.values)
        assertFalse(line.projected)
        assertEquals(listOf(2), n.known)
        assertEquals(listOf(Triple(1, 6129025L, true)), n.marks)
        assertFalse(n.fromZero)
    }

    @Test
    fun aLoneProjectionReadsAsOneWhenTapped() {
        val alone = readWritten(data("""{"labels": ["Mar", "Apr"], "series": [{"name": "Balance", "values": [6000, 6600], "projected": true}]}"""), cad)
        assertEquals("Balance (projected) $6,600.00", alone.parts(1, ", "))
        val actual = readWritten(data("""{"labels": ["Mar", "Apr"], "series": [{"name": "Balance", "values": [6000, 6600]}]}"""), cad)
        assertEquals("$6,600.00", actual.parts(1, ", "))
    }

    @Test
    fun aChartSaysWhichCopyItsNumbersCameFrom() {
        val spec = chartSpec("""{"type": "bar", "title": "Spending", "query": {"measure": "spending"}}""")!!
        val at = System.currentTimeMillis()
        val read = JsonObject(emptyMap())
        assertEquals("From your server's copy · just now", footNote(spec, RelayData.Live(read, at, fresh = false, cloud = true)))
        assertEquals("Saved on this phone · just now", footNote(spec, RelayData.Live(read, at, fresh = false)))
        assertEquals("Live from Tally, redraws when the ledger changes", footNote(spec, RelayData.Live(read, at, fresh = true)))
    }

    @Test
    fun aChartBlockWithItsOwnNumbersIsOneToDraw() {
        val spec = chartSpec("""{"type": "line", "title": "The next six months", "data": {"labels": ["Nov"], "series": [{"name": "Savings", "values": [1.5]}]}}""")
        assertNotNull(spec?.data)
        assertNull(spec?.query)
        assertEquals(ChartKind.Line, spec?.kind)
    }
}
