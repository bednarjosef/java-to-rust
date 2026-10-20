# Compares the JDK's sweep with the runtime's. Input: `paste` of the two files, so each line holds
# the JDK's fields and then the runtime's. Counts the chars where each method disagrees, prints the
# first few per method, and writes every disagreement to the file named by `detail`.
#
# Usage: paste java.txt rust.txt | awk -v detail=mismatches.txt -f summarise.awk

BEGIN { limit = 5 }

NR == 1 {
    # The header: "# code isDigit ..." on each side. Field j of a data line is header field j + 1.
    n = NF / 2
    for (j = 1; j < n; j++) name[j] = $(j + 1)
    next
}

{
    n = NF / 2
    checked++
    bad = 0
    for (j = 1; j <= n; j++) {
        if ($j == $(n + j)) continue
        bad = 1
        count[j]++
        line = sprintf("%s %s: JDK %s, runtime %s", $1, name[j], $j, $(n + j))
        if (count[j] <= limit) print "  " line
        print line > detail
    }
    if (bad) chars_bad++
}

END {
    printf "%d chars checked, %d with a disagreement\n", checked, chars_bad
    for (j = 1; j < n; j++) {
        if (count[j]) printf "  %-20s %6d chars\n", name[j], count[j]
    }
}
