package com.example.app;

/** Package-private, used from `Main` without an import. */
class Report {
    static String line(String label, int value) {
        return label + ": " + value;
    }

    static String describe() {
        return "report";
    }
}
