// Calls through one interface that 100 lambdas implement, three of them nearly every time, each
// recursing through the interface again with a point of its own that never leaves it, as the
// Minecraft server's codecs call `Function.apply` through thousands of lambdas, recursively.
// A dispatcher over that many classes keeps a small frame however its calls are optimized.
public class CallsThroughManyLambdas {
    interface Step {
        long run(int depth, long acc);
    }

    record Point(long x, long y, long z) {
        Point plus(Point o) {
            return new Point(x + o.x, y * 3 + o.y, z ^ o.z);
        }
    }

    // A named class among the lambdas, as the JDK's own classes are among `Function`'s.
    static final class Last implements Step {
        public long run(int depth, long acc) {
            return acc + depth;
        }
    }

    static final Step[] STEPS = new Step[100];

    static long call(int depth, long acc) {
        if (depth == 0) {
            return acc;
        }
        int pick = (int) ((acc >>> 7) % 97);
        Step step = pick == 96 && depth < 3 ? new Last() : STEPS[pick < 94 ? (int) (acc & 1) + (pick & 1) : pick];
        return step.run(depth - 1, acc);
    }

    static void fill() {
        STEPS[0] = (d, a) -> {
            Point p = new Point(a, d, 0);
            Point q = p.plus(new Point(1, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 0) & 0xFFFFFFFFFFL;
        };
        STEPS[1] = (d, a) -> {
            Point p = new Point(a, d, 1);
            Point q = p.plus(new Point(8, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 1) & 0xFFFFFFFFFFL;
        };
        STEPS[2] = (d, a) -> {
            Point p = new Point(a, d, 2);
            Point q = p.plus(new Point(15, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 2) & 0xFFFFFFFFFFL;
        };
        STEPS[3] = (d, a) -> {
            Point p = new Point(a, d, 3);
            Point q = p.plus(new Point(22, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 3) & 0xFFFFFFFFFFL;
        };
        STEPS[4] = (d, a) -> {
            Point p = new Point(a, d, 4);
            Point q = p.plus(new Point(29, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 4) & 0xFFFFFFFFFFL;
        };
        STEPS[5] = (d, a) -> {
            Point p = new Point(a, d, 5);
            Point q = p.plus(new Point(36, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 5) & 0xFFFFFFFFFFL;
        };
        STEPS[6] = (d, a) -> {
            Point p = new Point(a, d, 6);
            Point q = p.plus(new Point(43, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 6) & 0xFFFFFFFFFFL;
        };
        STEPS[7] = (d, a) -> {
            Point p = new Point(a, d, 7);
            Point q = p.plus(new Point(50, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 7) & 0xFFFFFFFFFFL;
        };
        STEPS[8] = (d, a) -> {
            Point p = new Point(a, d, 8);
            Point q = p.plus(new Point(57, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 8) & 0xFFFFFFFFFFL;
        };
        STEPS[9] = (d, a) -> {
            Point p = new Point(a, d, 9);
            Point q = p.plus(new Point(64, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 9) & 0xFFFFFFFFFFL;
        };
        STEPS[10] = (d, a) -> {
            Point p = new Point(a, d, 10);
            Point q = p.plus(new Point(71, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 10) & 0xFFFFFFFFFFL;
        };
        STEPS[11] = (d, a) -> {
            Point p = new Point(a, d, 11);
            Point q = p.plus(new Point(78, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 11) & 0xFFFFFFFFFFL;
        };
        STEPS[12] = (d, a) -> {
            Point p = new Point(a, d, 12);
            Point q = p.plus(new Point(85, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 12) & 0xFFFFFFFFFFL;
        };
        STEPS[13] = (d, a) -> {
            Point p = new Point(a, d, 13);
            Point q = p.plus(new Point(92, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 13) & 0xFFFFFFFFFFL;
        };
        STEPS[14] = (d, a) -> {
            Point p = new Point(a, d, 14);
            Point q = p.plus(new Point(99, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 14) & 0xFFFFFFFFFFL;
        };
        STEPS[15] = (d, a) -> {
            Point p = new Point(a, d, 15);
            Point q = p.plus(new Point(106, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 15) & 0xFFFFFFFFFFL;
        };
        STEPS[16] = (d, a) -> {
            Point p = new Point(a, d, 16);
            Point q = p.plus(new Point(113, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 16) & 0xFFFFFFFFFFL;
        };
        STEPS[17] = (d, a) -> {
            Point p = new Point(a, d, 17);
            Point q = p.plus(new Point(120, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 17) & 0xFFFFFFFFFFL;
        };
        STEPS[18] = (d, a) -> {
            Point p = new Point(a, d, 18);
            Point q = p.plus(new Point(127, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 18) & 0xFFFFFFFFFFL;
        };
        STEPS[19] = (d, a) -> {
            Point p = new Point(a, d, 19);
            Point q = p.plus(new Point(134, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 19) & 0xFFFFFFFFFFL;
        };
        STEPS[20] = (d, a) -> {
            Point p = new Point(a, d, 20);
            Point q = p.plus(new Point(141, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 20) & 0xFFFFFFFFFFL;
        };
        STEPS[21] = (d, a) -> {
            Point p = new Point(a, d, 21);
            Point q = p.plus(new Point(148, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 21) & 0xFFFFFFFFFFL;
        };
        STEPS[22] = (d, a) -> {
            Point p = new Point(a, d, 22);
            Point q = p.plus(new Point(155, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 22) & 0xFFFFFFFFFFL;
        };
        STEPS[23] = (d, a) -> {
            Point p = new Point(a, d, 23);
            Point q = p.plus(new Point(162, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 23) & 0xFFFFFFFFFFL;
        };
        STEPS[24] = (d, a) -> {
            Point p = new Point(a, d, 24);
            Point q = p.plus(new Point(169, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 24) & 0xFFFFFFFFFFL;
        };
        STEPS[25] = (d, a) -> {
            Point p = new Point(a, d, 25);
            Point q = p.plus(new Point(176, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 25) & 0xFFFFFFFFFFL;
        };
        STEPS[26] = (d, a) -> {
            Point p = new Point(a, d, 26);
            Point q = p.plus(new Point(183, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 26) & 0xFFFFFFFFFFL;
        };
        STEPS[27] = (d, a) -> {
            Point p = new Point(a, d, 27);
            Point q = p.plus(new Point(190, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 27) & 0xFFFFFFFFFFL;
        };
        STEPS[28] = (d, a) -> {
            Point p = new Point(a, d, 28);
            Point q = p.plus(new Point(197, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 28) & 0xFFFFFFFFFFL;
        };
        STEPS[29] = (d, a) -> {
            Point p = new Point(a, d, 29);
            Point q = p.plus(new Point(204, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 29) & 0xFFFFFFFFFFL;
        };
        STEPS[30] = (d, a) -> {
            Point p = new Point(a, d, 30);
            Point q = p.plus(new Point(211, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 30) & 0xFFFFFFFFFFL;
        };
        STEPS[31] = (d, a) -> {
            Point p = new Point(a, d, 31);
            Point q = p.plus(new Point(218, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 31) & 0xFFFFFFFFFFL;
        };
        STEPS[32] = (d, a) -> {
            Point p = new Point(a, d, 32);
            Point q = p.plus(new Point(225, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 32) & 0xFFFFFFFFFFL;
        };
        STEPS[33] = (d, a) -> {
            Point p = new Point(a, d, 33);
            Point q = p.plus(new Point(232, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 33) & 0xFFFFFFFFFFL;
        };
        STEPS[34] = (d, a) -> {
            Point p = new Point(a, d, 34);
            Point q = p.plus(new Point(239, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 34) & 0xFFFFFFFFFFL;
        };
        STEPS[35] = (d, a) -> {
            Point p = new Point(a, d, 35);
            Point q = p.plus(new Point(246, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 35) & 0xFFFFFFFFFFL;
        };
        STEPS[36] = (d, a) -> {
            Point p = new Point(a, d, 36);
            Point q = p.plus(new Point(253, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 36) & 0xFFFFFFFFFFL;
        };
        STEPS[37] = (d, a) -> {
            Point p = new Point(a, d, 37);
            Point q = p.plus(new Point(260, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 37) & 0xFFFFFFFFFFL;
        };
        STEPS[38] = (d, a) -> {
            Point p = new Point(a, d, 38);
            Point q = p.plus(new Point(267, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 38) & 0xFFFFFFFFFFL;
        };
        STEPS[39] = (d, a) -> {
            Point p = new Point(a, d, 39);
            Point q = p.plus(new Point(274, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 39) & 0xFFFFFFFFFFL;
        };
        STEPS[40] = (d, a) -> {
            Point p = new Point(a, d, 40);
            Point q = p.plus(new Point(281, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 40) & 0xFFFFFFFFFFL;
        };
        STEPS[41] = (d, a) -> {
            Point p = new Point(a, d, 41);
            Point q = p.plus(new Point(288, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 41) & 0xFFFFFFFFFFL;
        };
        STEPS[42] = (d, a) -> {
            Point p = new Point(a, d, 42);
            Point q = p.plus(new Point(295, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 42) & 0xFFFFFFFFFFL;
        };
        STEPS[43] = (d, a) -> {
            Point p = new Point(a, d, 43);
            Point q = p.plus(new Point(302, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 43) & 0xFFFFFFFFFFL;
        };
        STEPS[44] = (d, a) -> {
            Point p = new Point(a, d, 44);
            Point q = p.plus(new Point(309, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 44) & 0xFFFFFFFFFFL;
        };
        STEPS[45] = (d, a) -> {
            Point p = new Point(a, d, 45);
            Point q = p.plus(new Point(316, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 45) & 0xFFFFFFFFFFL;
        };
        STEPS[46] = (d, a) -> {
            Point p = new Point(a, d, 46);
            Point q = p.plus(new Point(323, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 46) & 0xFFFFFFFFFFL;
        };
        STEPS[47] = (d, a) -> {
            Point p = new Point(a, d, 47);
            Point q = p.plus(new Point(330, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 47) & 0xFFFFFFFFFFL;
        };
        STEPS[48] = (d, a) -> {
            Point p = new Point(a, d, 48);
            Point q = p.plus(new Point(337, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 48) & 0xFFFFFFFFFFL;
        };
        STEPS[49] = (d, a) -> {
            Point p = new Point(a, d, 49);
            Point q = p.plus(new Point(344, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 49) & 0xFFFFFFFFFFL;
        };
        STEPS[50] = (d, a) -> {
            Point p = new Point(a, d, 50);
            Point q = p.plus(new Point(351, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 50) & 0xFFFFFFFFFFL;
        };
        STEPS[51] = (d, a) -> {
            Point p = new Point(a, d, 51);
            Point q = p.plus(new Point(358, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 51) & 0xFFFFFFFFFFL;
        };
        STEPS[52] = (d, a) -> {
            Point p = new Point(a, d, 52);
            Point q = p.plus(new Point(365, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 52) & 0xFFFFFFFFFFL;
        };
        STEPS[53] = (d, a) -> {
            Point p = new Point(a, d, 53);
            Point q = p.plus(new Point(372, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 53) & 0xFFFFFFFFFFL;
        };
        STEPS[54] = (d, a) -> {
            Point p = new Point(a, d, 54);
            Point q = p.plus(new Point(379, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 54) & 0xFFFFFFFFFFL;
        };
        STEPS[55] = (d, a) -> {
            Point p = new Point(a, d, 55);
            Point q = p.plus(new Point(386, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 55) & 0xFFFFFFFFFFL;
        };
        STEPS[56] = (d, a) -> {
            Point p = new Point(a, d, 56);
            Point q = p.plus(new Point(393, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 56) & 0xFFFFFFFFFFL;
        };
        STEPS[57] = (d, a) -> {
            Point p = new Point(a, d, 57);
            Point q = p.plus(new Point(400, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 57) & 0xFFFFFFFFFFL;
        };
        STEPS[58] = (d, a) -> {
            Point p = new Point(a, d, 58);
            Point q = p.plus(new Point(407, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 58) & 0xFFFFFFFFFFL;
        };
        STEPS[59] = (d, a) -> {
            Point p = new Point(a, d, 59);
            Point q = p.plus(new Point(414, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 59) & 0xFFFFFFFFFFL;
        };
        STEPS[60] = (d, a) -> {
            Point p = new Point(a, d, 60);
            Point q = p.plus(new Point(421, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 60) & 0xFFFFFFFFFFL;
        };
        STEPS[61] = (d, a) -> {
            Point p = new Point(a, d, 61);
            Point q = p.plus(new Point(428, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 61) & 0xFFFFFFFFFFL;
        };
        STEPS[62] = (d, a) -> {
            Point p = new Point(a, d, 62);
            Point q = p.plus(new Point(435, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 62) & 0xFFFFFFFFFFL;
        };
        STEPS[63] = (d, a) -> {
            Point p = new Point(a, d, 63);
            Point q = p.plus(new Point(442, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 63) & 0xFFFFFFFFFFL;
        };
        STEPS[64] = (d, a) -> {
            Point p = new Point(a, d, 64);
            Point q = p.plus(new Point(449, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 64) & 0xFFFFFFFFFFL;
        };
        STEPS[65] = (d, a) -> {
            Point p = new Point(a, d, 65);
            Point q = p.plus(new Point(456, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 65) & 0xFFFFFFFFFFL;
        };
        STEPS[66] = (d, a) -> {
            Point p = new Point(a, d, 66);
            Point q = p.plus(new Point(463, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 66) & 0xFFFFFFFFFFL;
        };
        STEPS[67] = (d, a) -> {
            Point p = new Point(a, d, 67);
            Point q = p.plus(new Point(470, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 67) & 0xFFFFFFFFFFL;
        };
        STEPS[68] = (d, a) -> {
            Point p = new Point(a, d, 68);
            Point q = p.plus(new Point(477, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 68) & 0xFFFFFFFFFFL;
        };
        STEPS[69] = (d, a) -> {
            Point p = new Point(a, d, 69);
            Point q = p.plus(new Point(484, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 69) & 0xFFFFFFFFFFL;
        };
        STEPS[70] = (d, a) -> {
            Point p = new Point(a, d, 70);
            Point q = p.plus(new Point(491, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 70) & 0xFFFFFFFFFFL;
        };
        STEPS[71] = (d, a) -> {
            Point p = new Point(a, d, 71);
            Point q = p.plus(new Point(498, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 71) & 0xFFFFFFFFFFL;
        };
        STEPS[72] = (d, a) -> {
            Point p = new Point(a, d, 72);
            Point q = p.plus(new Point(505, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 72) & 0xFFFFFFFFFFL;
        };
        STEPS[73] = (d, a) -> {
            Point p = new Point(a, d, 73);
            Point q = p.plus(new Point(512, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 73) & 0xFFFFFFFFFFL;
        };
        STEPS[74] = (d, a) -> {
            Point p = new Point(a, d, 74);
            Point q = p.plus(new Point(519, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 74) & 0xFFFFFFFFFFL;
        };
        STEPS[75] = (d, a) -> {
            Point p = new Point(a, d, 75);
            Point q = p.plus(new Point(526, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 75) & 0xFFFFFFFFFFL;
        };
        STEPS[76] = (d, a) -> {
            Point p = new Point(a, d, 76);
            Point q = p.plus(new Point(533, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 76) & 0xFFFFFFFFFFL;
        };
        STEPS[77] = (d, a) -> {
            Point p = new Point(a, d, 77);
            Point q = p.plus(new Point(540, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 77) & 0xFFFFFFFFFFL;
        };
        STEPS[78] = (d, a) -> {
            Point p = new Point(a, d, 78);
            Point q = p.plus(new Point(547, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 78) & 0xFFFFFFFFFFL;
        };
        STEPS[79] = (d, a) -> {
            Point p = new Point(a, d, 79);
            Point q = p.plus(new Point(554, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 79) & 0xFFFFFFFFFFL;
        };
        STEPS[80] = (d, a) -> {
            Point p = new Point(a, d, 80);
            Point q = p.plus(new Point(561, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 80) & 0xFFFFFFFFFFL;
        };
        STEPS[81] = (d, a) -> {
            Point p = new Point(a, d, 81);
            Point q = p.plus(new Point(568, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 81) & 0xFFFFFFFFFFL;
        };
        STEPS[82] = (d, a) -> {
            Point p = new Point(a, d, 82);
            Point q = p.plus(new Point(575, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 82) & 0xFFFFFFFFFFL;
        };
        STEPS[83] = (d, a) -> {
            Point p = new Point(a, d, 83);
            Point q = p.plus(new Point(582, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 83) & 0xFFFFFFFFFFL;
        };
        STEPS[84] = (d, a) -> {
            Point p = new Point(a, d, 84);
            Point q = p.plus(new Point(589, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 84) & 0xFFFFFFFFFFL;
        };
        STEPS[85] = (d, a) -> {
            Point p = new Point(a, d, 85);
            Point q = p.plus(new Point(596, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 85) & 0xFFFFFFFFFFL;
        };
        STEPS[86] = (d, a) -> {
            Point p = new Point(a, d, 86);
            Point q = p.plus(new Point(603, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 86) & 0xFFFFFFFFFFL;
        };
        STEPS[87] = (d, a) -> {
            Point p = new Point(a, d, 87);
            Point q = p.plus(new Point(610, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 87) & 0xFFFFFFFFFFL;
        };
        STEPS[88] = (d, a) -> {
            Point p = new Point(a, d, 88);
            Point q = p.plus(new Point(617, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 88) & 0xFFFFFFFFFFL;
        };
        STEPS[89] = (d, a) -> {
            Point p = new Point(a, d, 89);
            Point q = p.plus(new Point(624, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 89) & 0xFFFFFFFFFFL;
        };
        STEPS[90] = (d, a) -> {
            Point p = new Point(a, d, 90);
            Point q = p.plus(new Point(631, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 90) & 0xFFFFFFFFFFL;
        };
        STEPS[91] = (d, a) -> {
            Point p = new Point(a, d, 91);
            Point q = p.plus(new Point(638, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 91) & 0xFFFFFFFFFFL;
        };
        STEPS[92] = (d, a) -> {
            Point p = new Point(a, d, 92);
            Point q = p.plus(new Point(645, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 92) & 0xFFFFFFFFFFL;
        };
        STEPS[93] = (d, a) -> {
            Point p = new Point(a, d, 93);
            Point q = p.plus(new Point(652, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 93) & 0xFFFFFFFFFFL;
        };
        STEPS[94] = (d, a) -> {
            Point p = new Point(a, d, 94);
            Point q = p.plus(new Point(659, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 94) & 0xFFFFFFFFFFL;
        };
        STEPS[95] = (d, a) -> {
            Point p = new Point(a, d, 95);
            Point q = p.plus(new Point(666, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 95) & 0xFFFFFFFFFFL;
        };
        STEPS[96] = (d, a) -> {
            Point p = new Point(a, d, 96);
            Point q = p.plus(new Point(673, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 96) & 0xFFFFFFFFFFL;
        };
        STEPS[97] = (d, a) -> {
            Point p = new Point(a, d, 97);
            Point q = p.plus(new Point(680, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 97) & 0xFFFFFFFFFFL;
        };
        STEPS[98] = (d, a) -> {
            Point p = new Point(a, d, 98);
            Point q = p.plus(new Point(687, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 98) & 0xFFFFFFFFFFL;
        };
        STEPS[99] = (d, a) -> {
            Point p = new Point(a, d, 99);
            Point q = p.plus(new Point(694, a >>> 3, d));
            return call(d, q.x() * 31 + q.y() + q.z() + 99) & 0xFFFFFFFFFFL;
        };
    }

    public static void main(String[] args) {
        fill();
        long sum = 0;
        for (int i = 0; i < 20000; i++) {
            sum += call(200 + i % 50, i);
        }
        System.out.println("dispatch frames " + sum + " " + call(4000, 1));
    }
}
