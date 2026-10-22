public class NullClassNames {
  public static void main(String[] a) {
    String n = a.length > 5 ? "x" : null;
    try { Class.forName(n); } catch (Exception e) { System.out.println(e.getMessage()); }
    try { Class.forName(n, false, NullClassNames.class.getClassLoader()); } catch (Exception e) { System.out.println(e.getMessage()); }
    try { ClassLoader.getSystemClassLoader().loadClass(n); } catch (Exception e) { System.out.println(e); }
  }
}
