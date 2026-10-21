package com.bank.core;

/** An account that earns interest and charges a fee only when the balance is low. */
public class Savings extends Account {
    private final int ratePercent;

    public Savings(String owner, int ratePercent) {
        super(owner);
        this.ratePercent = ratePercent;
    }

    @Override
    public long fee() {
        return balance < 1000 ? 5 : 0;
    }

    @Override
    public void monthEnd() {
        balance += balance * ratePercent / 100;
        super.monthEnd();
    }
}
