"""Financial commands: every form, interval boundaries, and degenerate
inputs (zero and negative rates, negative periods, a missing guess)."""

from caselib import Cases, setn

cases = Cases("financial")

# Rate: solved for, in each of its four forms (3, 4, 5 and 6 arguments).
cases.case("rate", [
    "r = Rate(4, -200, 1000)",
], [])
cases.case("rate", [
    "r = Rate(4, -200, 1000, 0)",
], [])
cases.case("rate", [
    "r = Rate(4, -200, 1000, 0, 0)",
], [])
cases.case("rate", [
    "r = Rate(4, -200, 1000, 0, 0, 0.1)",
], [])
cases.case("rate", [
    "n = 4", "r = Rate(n, -200, 1000)",
], [setn("n", 12), setn("n", 1), setn("n", 4)])
# a zero rate is solved without the root finder
cases.case("rate", [
    "r = Rate(10, -100, 1000)",
], [])
# an annuity: paying back 5 a month for 12 months on 55
cases.case("rate", [
    "r = Rate(12, -5, 55)",
], [])
# payments at the start of the period
cases.case("rate", [
    "r = Rate(12, -5, 55, 1)",
], [])
# a final value besides the present one
cases.case("rate", [
    "r = Rate(4, -200, 1000, 0, 1000)",
], [])
# a guess that sends the search elsewhere
cases.case("rate", [
    "r = Rate(12, -5, 55, 0, 0, 0.5)",
], [])
# a negative number of periods is undefined
cases.case("rate", [
    "r = Rate(-4, -200, 1000)",
], [])
# one period, and the boundary where nothing is left
cases.case("rate", [
    "r = Rate(1, -1000, 1000)",
], [])

# Periods: how long paying off takes, with and without a final value and
# a guess.
cases.case("periods", [
    "n = Periods(0.01, -200, 1000)",
], [])
cases.case("periods", [
    "n = Periods(0.01, -200, 1000, 0)",
], [])
cases.case("periods", [
    "n = Periods(0.01, -200, 1000, 0, 0)",
], [])
cases.case("periods", [
    "r = 0.005", "n = Periods(r, -100, 5000)",
], [setn("r", 0.01), setn("r", 0.05), setn("r", 0.1), setn("r", 0.0)])
# a zero rate divides instead of compounding
cases.case("periods", [
    "n = Periods(0, -100, 1000)",
], [])
# nothing to pay off
cases.case("periods", [
    "n = Periods(0.01, -100, 0)",
], [])
# a final value to reach by saving
cases.case("periods", [
    "n = Periods(0.05, -100, 0, 0, 1000)",
], [])

# Payment: the instalment of a loan and the withdrawal of a deposit.
cases.case("payment", [
    "p = Payment(10, 0.05, 1000)",
], [])
cases.case("payment", [
    "p = Payment(10, 0.05, 1000, 0)",
], [])
cases.case("payment", [
    "p = Payment(10, 0.05, 1000, 0, 0)",
], [])
cases.case("payment", [
    "r = 0.04", "p = Payment(24, r, 5000)",
], [setn("r", 0.0), setn("r", 0.08), setn("r", 0.5), setn("r", -0.5)])
# payments at the start of the period
cases.case("payment", [
    "p = Payment(10, 0.05, 1000, 1)",
], [])
# a zero rate is a plain division
cases.case("payment", [
    "p = Payment(10, 0, 1000)",
], [])
# saving up to a final value from nothing
cases.case("payment", [
    "p = Payment(10, 0.05, 0, 0, 1000)",
], [])
# nothing to pay
cases.case("payment", [
    "p = Payment(10, 0.05, 0)",
], [])

# FutureValue: what a stream of payments grows to.
cases.case("future", [
    "f = FutureValue(10, 0.05, -100)",
], [])
cases.case("future", [
    "f = FutureValue(10, 0.05, -100, 0)",
], [])
cases.case("future", [
    "f = FutureValue(10, 0.05, -100, 0, 500)",
], [])
cases.case("future", [
    "r = 0.03", "f = FutureValue(12, r, -250)",
], [setn("r", 0.0), setn("r", 0.25), setn("r", -0.25), setn("r", 0.03)])
# payments at the start of the period
cases.case("future", [
    "f = FutureValue(10, 0.05, -100, 1)",
], [])
# a zero rate adds up
cases.case("future", [
    "f = FutureValue(10, 0, -100)",
], [])
# a fractional number of periods
cases.case("future", [
    "f = FutureValue(4.5, 0.07, -13)",
], [])
# a negative number of periods is undefined
cases.case("future", [
    "f = FutureValue(-3, 0.07, -13)",
], [])
# an overflowing input is an undefined number, which the setters reject
cases.case("future", [
    "f = FutureValue(0, 4, 1/0, 13)",
], [])
cases.case("future", [
    "f = FutureValue(0, 4, 10^400, 13)",
], [])

# PresentValue: what the stream is worth today.
cases.case("present", [
    "p = PresentValue(10, 0.05, -100)",
], [])
cases.case("present", [
    "p = PresentValue(10, 0.05, -100, 0)",
], [])
cases.case("present", [
    "p = PresentValue(10, 0.05, -100, 0, 500)",
], [])
cases.case("present", [
    "r = 0.02", "p = PresentValue(36, r, -150)",
], [setn("r", 0.0), setn("r", 0.4), setn("r", -0.4), setn("r", 0.02)])
# payments at the start of the period
cases.case("present", [
    "p = PresentValue(10, 0.05, -100, 1)",
], [])
# a zero rate adds up
cases.case("present", [
    "p = PresentValue(10, 0, -100)",
], [])

# nCr and nPr: whole, non-integer, huge and out-of-range arguments.
cases.case("ncr", [
    "a = nCr(5, 2)", "b = nCr(5, 0)", "c = nCr(5, 5)", "d = nCr(5, 7)",
    "e = nCr(5, -1)",
], [])
cases.case("ncr", [
    "a = nCr(10, 4)", "b = nCr(20, 10)", "c = nCr(37, 18)",
], [])
cases.case("ncr", [
    "a = nCr(2.5, 2)", "b = nCr(5, 2.5)", "c = nCr(-3, 2)",
], [])
cases.case("ncr", [
    "a = nCr(1000000, 2)", "b = nCr(1000000, 3)",
], [])
cases.case("npr", [
    "a = nPr(8, 7)", "b = nPr(5, 0)", "c = nPr(5, 6)", "d = nPr(5, -1)",
], [])
cases.case("npr", [
    "a = nPr(10, 4)", "b = nPr(16, 8)",
], [])
cases.case("npr", [
    "a = nPr(2.5, 2)", "b = nPr(5, 2.5)",
], [])
cases.case("npr", [
    "a = nPr(1000000, 3)",
], [])
# beyond 128 bits: the decimal of n - r equals that of n, so the product
# of the factors is empty
cases.case("npr", [
    "a = nPr(10^40, 2)", "b = nPr(10^300, 0)",
], [])
# nCr in the same range keeps the log value
cases.case("ncr", [
    "a = nCr(10^40, 2)",
], [])

cases.write()
