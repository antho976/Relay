package com.tally.app.data

/** Wealthsimple's files as the shared tests of docs/INVESTMENTS.md embed them (relay-money's wealthsimple.rs). */
object WsFiles {

    /** W1: the holdings report's header and a public fixture's demo rows, the AAPL row in USD. */
    val HOLDINGS_REPORT = """
        Account Name,Account Type,Account Classification,Account Number,Symbol,Exchange,MIC,Name,Security Type,Quantity,Position Direction,Market Price,Market Price Currency,Book Value (CAD),Book Value Currency (CAD),Book Value (Market),Book Value Currency (Market),Market Value,Market Value Currency,Market Unrealized Returns,Market Unrealized Returns Currency
        "Demo TFSA","TFSA","Trade","DEMO0001CAD","AAPL","NASDAQ","XNAS","Apple Inc","EQUITY","10","LONG","100","USD","1000","CAD","750","USD","1000","USD","0","USD"
        "Demo TFSA","TFSA","Trade","DEMO0001CAD","XEQT","TSX","XTSE","iShares Core Equity ETF Portfolio","EXCHANGE_TRADED_FUND","10","LONG","25","CAD","250","CAD","250","CAD","250","CAD","0","CAD"
        "Demo TFSA","TFSA","Trade","DEMO0001CAD","ARKK","BATS","BATS","ARK Innovation ETF","EXCHANGE_TRADED_FUND","1","LONG","50","USD","50","CAD","50","USD","50","USD","0","USD"

        "As of 2026-05-08 12:00 GMT-04:00"
    """.trimIndent() + "\n"

    /** W2: a contribution, a buy, an exchange's two legs, an option trade, a DRIP pair and a footer. */
    val ACTIVITIES_EXPORT = """
        transaction_date,settlement_date,account_id,account_type,activity_type,activity_sub_type,direction,symbol,name,currency,quantity,unit_price,commission,net_cash_amount
        2026-08-15,2026-08-15,HQ7XFMC41CAD,TFSA,MoneyMovement,CONTRIBUTION,,,,CAD,,,,500.00
        2026-08-18,2026-08-19,HQ7XFMC41CAD,TFSA,Trade,BUY,,XEQT,iShares Core Equity ETF Portfolio,CAD,10,38.12,0,-381.20
        2026-09-02,2026-09-02,HQ7XFMC41CAD,TFSA,FxExchange,,,,,CAD,,,,-32.93
        2026-09-02,2026-09-02,HQ7XFMC41CAD,TFSA,FxExchange,,,,,USD,,,,24.33
        2026-09-10,2026-09-11,HQ7XFMC41CAD,TFSA,Trade,STO,,AAPL 2026-10-16 250.00 C,Apple Inc call,USD,-1,1.25,0.75,124.25
        2026-09-29,2026-09-29,HQ7XFMC41CAD,TFSA,Dividend,,,XEQT,iShares Core Equity ETF Portfolio,CAD,,,,4.21
        2026-09-29,2026-09-29,HQ7XFMC41CAD,TFSA,Trade,DRIP,,XEQT,iShares Core Equity ETF Portfolio,CAD,0.1104,38.13,0,-4.21
        "Exported on 2026-10-09"
    """.trimIndent() + "\n"
}
