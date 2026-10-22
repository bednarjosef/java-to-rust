public class Seasons {
    enum Season { WINTER, SPRING, SUMMER, AUTUMN }

    static String describe(Season season) {
        switch (season) {
            case WINTER:
                return "cold";
            case SUMMER:
                return "warm";
            default:
                return "mild";
        }
    }

    static int months(Season season) {
        return switch (season) {
            case WINTER, SUMMER -> 3;
            case SPRING -> {
                int days = 92;
                yield days / 30;
            }
            case AUTUMN -> 3;
        };
    }

    public static void main(String[] args) {
        for (Season season : Season.values()) {
            System.out.println(season + " " + describe(season) + " " + months(season));
        }
    }
}
