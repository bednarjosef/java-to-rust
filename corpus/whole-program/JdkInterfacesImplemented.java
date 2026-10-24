// Classes of the program implementing plain interfaces of the JDK that are otherwise reflection's
// or security's: types of generics, a key and its specs, an action and an invocation handler,
// each called through its interface, with the interfaces' default methods.
import java.lang.reflect.GenericArrayType;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.lang.reflect.WildcardType;
import java.security.Key;
import java.security.PrivateKey;
import java.security.PrivilegedExceptionAction;
import java.security.spec.AlgorithmParameterSpec;
import java.security.spec.KeySpec;
import java.util.Arrays;
import javax.security.auth.DestroyFailedException;

public class JdkInterfacesImplemented {
    static final class Named implements Type {
        final String name;

        Named(String name) {
            this.name = name;
        }

        @Override
        public String toString() {
            return name;
        }
    }

    static final class Parameterized implements ParameterizedType {
        final Type raw;
        final Type[] args;

        Parameterized(Type raw, Type... args) {
            this.raw = raw;
            this.args = args.clone();
        }

        public Type[] getActualTypeArguments() {
            return args.clone();
        }

        public Type getRawType() {
            return raw;
        }

        public Type getOwnerType() {
            return null;
        }

        @Override
        public String toString() {
            StringBuilder out = new StringBuilder(raw.getTypeName()).append('<');
            for (int i = 0; i < args.length; i++) {
                out.append(i == 0 ? "" : ", ").append(args[i].getTypeName());
            }
            return out.append('>').toString();
        }
    }

    static final class Wildcard implements WildcardType {
        final Type upper;

        Wildcard(Type upper) {
            this.upper = upper;
        }

        public Type[] getUpperBounds() {
            return new Type[] {upper};
        }

        public Type[] getLowerBounds() {
            return new Type[0];
        }

        @Override
        public String toString() {
            return "? extends " + upper.getTypeName();
        }
    }

    static final class ArrayOf implements GenericArrayType {
        final Type component;

        ArrayOf(Type component) {
            this.component = component;
        }

        public Type getGenericComponentType() {
            return component;
        }

        @Override
        public String getTypeName() {
            return component.getTypeName() + "[]";
        }
    }

    static final class SecretKey implements PrivateKey {
        final byte[] bytes;

        SecretKey(byte[] bytes) {
            this.bytes = bytes;
        }

        public String getAlgorithm() {
            return "XOR";
        }

        public String getFormat() {
            return "RAW";
        }

        public byte[] getEncoded() {
            return bytes.clone();
        }
    }

    static final class Spec implements KeySpec, AlgorithmParameterSpec {
        @Override
        public String toString() {
            return "spec";
        }
    }

    static final class Echo implements InvocationHandler {
        public Object invoke(Object proxy, Method method, Object[] args) {
            return "invoked with " + Arrays.toString(args);
        }
    }

    static String describe(Type type) {
        if (type instanceof ParameterizedType p) {
            return "parameterized " + p.getRawType() + " of " + p.getActualTypeArguments().length;
        }
        if (type instanceof WildcardType w) {
            return "wildcard bounded by " + w.getUpperBounds()[0];
        }
        if (type instanceof GenericArrayType a) {
            return "array of " + a.getGenericComponentType();
        }
        return "named " + type;
    }

    static <T> T run(PrivilegedExceptionAction<T> action) {
        try {
            return action.run();
        } catch (Exception e) {
            System.out.println("failed: " + e.getMessage());
            return null;
        }
    }

    public static void main(String[] args) throws Throwable {
        Type string = new Named("java.lang.String");
        Type list = new Parameterized(new Named("java.util.List"), new Wildcard(string));
        Type[] types = {string, list, new Wildcard(string), new ArrayOf(list)};
        for (Type type : types) {
            System.out.println(type.getTypeName() + ": " + describe(type));
        }

        SecretKey secret = new SecretKey(new byte[] {1, 2, 3});
        Key key = secret;
        System.out.println(key.getAlgorithm() + " " + key.getFormat() + " " + key.getEncoded().length);
        System.out.println(secret.getParams() + " " + secret.isDestroyed());
        try {
            secret.destroy();
        } catch (DestroyFailedException e) {
            System.out.println("not destroyed: " + e.getMessage());
        }
        KeySpec spec = new Spec();
        AlgorithmParameterSpec parameters = (AlgorithmParameterSpec) spec;
        System.out.println(spec + " " + parameters);

        System.out.println(run(() -> 6 * 7));
        Integer none = run(() -> {
            throw new IllegalStateException("no value");
        });
        System.out.println(none);

        InvocationHandler handler = new Echo();
        System.out.println(handler.invoke(null, null, new Object[] {1, "two"}));
    }
}
