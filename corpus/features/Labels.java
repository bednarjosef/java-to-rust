public class Labels {
    public static void main(String[] args) {
        int found = -1;
        search:
        for (int i = 0; i < 6; i++) {
            for (int j = 0; j < 6; j++) {
                if (i * j == 12) {
                    found = i * 10 + j;
                    break search;
                }
            }
        }
        System.out.println("found " + found);

        int pairs = 0;
        outer:
        for (int i = 0; i < 5; i++) {
            for (int j = 0; j < 5; j++) {
                if (j > i) {
                    continue outer;
                }
                pairs++;
            }
        }
        System.out.println("pairs " + pairs);

        int n = 0;
        scan:
        while (n < 100) {
            n += 7;
            for (int k = 0; k < 3; k++) {
                if (n % 5 == 0) {
                    break scan;
                }
                if (k == 1) {
                    continue scan;
                }
            }
        }
        System.out.println("n " + n);

        int m = 0;
        again:
        do {
            m++;
            if (m < 4) {
                continue again;
            }
        } while (m < 6);
        System.out.println("m " + m);

        int stage = 0;
        block: {
            stage = 1;
            if (stage == 1) {
                break block;
            }
            stage = 2;
        }
        System.out.println("stage " + stage);

        int hits = 0;
        for (int i = 0; i < 4; i++) {
            loop:
            for (int j = 0; j < 4; j++) {
                switch (j) {
                    case 0:
                        continue loop;
                    case 2:
                        break loop;
                    default:
                        hits += 10;
                }
                hits += 1;
            }
        }
        System.out.println("hits " + hits);

        int total = 0;
        rows:
        for (int r = 0; r < 4; r++) {
            switch (r) {
                case 1 -> {
                    continue rows;
                }
                case 3 -> {
                    break rows;
                }
                default -> total += r;
            }
            total += 100;
        }
        System.out.println("total " + total);

        int plain = 0;
        for (int i = 0; i < 5; i++) {
            if (i == 3) {
                break;
            }
            plain += i;
        }
        System.out.println("plain " + plain);
    }
}
