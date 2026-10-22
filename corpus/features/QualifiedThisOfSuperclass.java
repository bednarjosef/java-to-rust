// `Outer.this` in an anonymous subclass of `Outer` made in `Outer`'s code is the enclosing
// instance, not the anonymous object, as in javac's Type.JCPrimitiveType.cloneWithMetadata.
public class QualifiedThisOfSuperclass {
    static class Type {
        final String name;

        Type(String name) {
            this.name = name;
        }

        Type baseType() {
            return this;
        }

        Type withMetadata(String metadata) {
            return new Type(name + "@" + metadata) {
                Type baseType() {
                    return Type.this.baseType();
                }
            };
        }
    }

    public static void main(String[] args) {
        Type t = new Type("int");
        Type annotated = t.withMetadata("NonNull").withMetadata("Old");
        System.out.println(annotated.name + " " + annotated.baseType().name + " " + (annotated.baseType() == t));
    }
}
