// Objects made from a `Class<? extends T>` that holds a class literal passed to a constructor, as
// the JDK's `KeyStoreDelegator` makes its key store (`primaryKeyStore.newInstance()` of a
// `Class<? extends KeyStoreSpi>` that `PKCS12KeyStore.class` was passed for). A class that is not
// a `Spi` but has a literal in the program is no candidate.
abstract class Spi {
    abstract String name();
}

class FirstSpi extends Spi {
    String name() {
        return "first";
    }
}

class SecondSpi extends Spi {
    public SecondSpi() {
        System.out.println("making the second");
    }

    String name() {
        return "second";
    }
}

class Unrelated {
    Unrelated() {
        System.out.println("never made");
    }
}

class Delegator {
    private final Class<? extends Spi> primary;
    private final Class<? extends Spi> secondary;

    Delegator(Class<? extends Spi> primary, Class<? extends Spi> secondary) {
        this.primary = primary;
        this.secondary = secondary;
    }

    @SuppressWarnings("deprecation")
    Spi make(boolean second) throws ReflectiveOperationException {
        return second ? secondary.getDeclaredConstructor().newInstance() : primary.newInstance();
    }
}

public class BoundedClassNewInstance {
    public static void main(String[] args) throws Exception {
        Delegator delegator = new Delegator(FirstSpi.class, SecondSpi.class);
        System.out.println(delegator.make(false).name());
        System.out.println(delegator.make(true).name());
        System.out.println(Unrelated.class.getSimpleName());
    }
}
