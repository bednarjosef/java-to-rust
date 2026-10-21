package shop;

import java.io.ObjectStreamField;
import java.io.Serializable;

/**
 * A serializable class that names its serialized fields, as `java.util.Random` and `String` do:
 * only serialization reads `serialPersistentFields`, by reflection, so the class's
 * initialization does not create the `ObjectStreamField`s.
 */
public class Main {
    static class Price implements Serializable {
        private static final long serialVersionUID = 1L;
        private static final ObjectStreamField[] serialPersistentFields = {
            new ObjectStreamField("cents", long.class), new ObjectStreamField("currency", String.class)
        };
        static int made = 0;

        final long cents;
        final String currency;

        Price(long cents, String currency) {
            this.cents = cents;
            this.currency = currency;
            made++;
        }

        public String toString() {
            return cents / 100 + "." + (cents % 100 < 10 ? "0" : "") + cents % 100 + " " + currency;
        }
    }

    public static void main(String[] args) {
        System.out.println(new Price(1999, "EUR"));
        System.out.println(new Price(5, "CZK"));
        System.out.println(Price.made);
    }
}
