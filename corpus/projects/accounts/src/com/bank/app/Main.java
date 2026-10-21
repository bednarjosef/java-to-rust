package com.bank.app;

import com.bank.core.Account;
import com.bank.core.Savings;

/** A superclass and its subclass in another package, used from a third. */
public class Main {
    static long total(Account[] accounts) {
        long sum = 0;
        for (Account a : accounts) {
            sum += a.balance();
        }
        return sum;
    }

    public static void main(String[] args) {
        Savings small = new Savings("ann", 2);
        Savings large = new Savings("ben", 3);
        small.deposit(500);
        large.deposit(5000);
        Account[] accounts = {small, large};
        for (int month = 0; month < 3; month++) {
            for (Account a : accounts) {
                a.monthEnd();
            }
        }
        for (Account a : accounts) {
            System.out.println(a + " fee " + a.fee() + " number " + a.number());
        }
        System.out.println(total(accounts) + " in " + Account.issued() + " accounts");
    }
}
