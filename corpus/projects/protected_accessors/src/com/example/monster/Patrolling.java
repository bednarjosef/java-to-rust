package com.example.monster;

/** A superclass whose protected members a nested class of a subclass in another package uses. */
public abstract class Patrolling {
    protected boolean patrolling = true;
    protected int rounds;

    protected boolean isPatrolling() {
        return patrolling;
    }

    protected static int count(Patrolling p) {
        return p.isPatrolling() ? 1 : 0;
    }
}
