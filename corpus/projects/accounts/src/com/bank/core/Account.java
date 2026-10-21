package com.bank.core;

/** The superclass of every account: its number, owner and balance, and the rules they share. */
public abstract class Account {
    private static int issued = 0;
    protected final String owner;
    protected long balance;
    private final int number;

    protected Account(String owner) {
        issued++;
        this.number = issued;
        this.owner = owner;
    }

    public int number() {
        return number;
    }

    public long balance() {
        return balance;
    }

    public void deposit(long amount) {
        balance += amount;
    }

    public abstract long fee();

    public void monthEnd() {
        balance -= fee();
    }

    public static int issued() {
        return issued;
    }

    @Override
    public String toString() {
        return "#" + number + " " + owner + " " + balance;
    }
}
