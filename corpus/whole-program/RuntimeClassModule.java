public class RuntimeClassModule {
    public static void main(String[] args) {
        Module module = Runtime.class.getModule();
        System.out.println(module.getName());
        ModuleLayer layer = module.getLayer();
        System.out.println(layer != null);
        System.out.println(layer.findModule("java.base").isPresent());
    }
}
