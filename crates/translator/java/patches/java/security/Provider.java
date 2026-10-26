package java.security;

import sun.security.jca.TranslatedServices;

/**
 * The translator's patch of the JDK's {@code Provider} (see docs/jdk-overlays.md). A service's
 * implementation is a class that the provider names, which the JDK finds by name and makes by
 * reflection; the translation makes it through {@code TranslatedServices.newInstance}, whose
 * classes are those the providers register for the services that reachable code looks up.
 */
@Edit(find = "Class<Policy.Parameters> policyParams = Policy.Parameters.class;",
        replace = "Class<?> policyParams = null;")
class Provider {
    // `Policy` is not translated, so neither is its parameters' class; a policy service, whose
    // implementation is not translated either, takes no parameter.

    @Patch
    public static class Service {
        // The implementation is made by its class's name, without `Class` and `Constructor`
        // objects: a class that is not translated is not found, and one without the constructor
        // fails as `getConstructor` would. An implementation of the time before parameters, with
        // a constructor without them only, is made with that one for a null parameter, as in
        // the JDK.
        @Replace
        private Object newInstanceUtil(Class<?> ctrParamClz, Object ctorParamObj) throws Exception {
            try {
                try {
                    return TranslatedServices.newInstance(className, ctrParamClz, ctorParamObj);
                } catch (NoSuchMethodException withParameter) {
                    if (ctrParamClz == null || ctorParamObj != null) {
                        throw withParameter;
                    }
                    try {
                        return TranslatedServices.newInstance(className, null, null);
                    } catch (NoSuchMethodException without) {
                        withParameter.addSuppressed(without);
                        throw withParameter;
                    }
                }
            } catch (ClassNotFoundException e) {
                throw new NoSuchAlgorithmException("class configured for " + type
                        + " (provider: " + provider.getName() + ") cannot be found.", e);
            }
        }

        @Remove
        private Object newInstanceOf();

        @Remove
        private Class<?> getImplClass();

        @Remove
        private Constructor<?> getDefaultConstructor();

        @Replace
        private Class<?> getKeyClass(String name) {
            return TranslatedServices.keyClass(name);
        }
    }
}
