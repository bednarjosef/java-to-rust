public class ArrayCloneNull {
    enum Kind { A, B }

    static class Inner {
    }

    int[] groups;
    String[][] names;
    Integer[] boxes;
    Kind[] kinds;
    Inner[] inners;
    Object[] objects;

    static void show(NullPointerException e) {
        System.out.println(e.getMessage());
    }

    public static void main(String[] args) {
        ArrayCloneNull a = new ArrayCloneNull();
        try {
            System.out.println(a.groups.clone().length);
        } catch (NullPointerException e) {
            show(e);
        }
        try {
            System.out.println(a.names.clone().length);
        } catch (NullPointerException e) {
            show(e);
        }
        try {
            System.out.println(a.boxes.clone().length);
        } catch (NullPointerException e) {
            show(e);
        }
        try {
            System.out.println(a.kinds.clone().length);
        } catch (NullPointerException e) {
            show(e);
        }
        try {
            System.out.println(a.inners.clone().length);
        } catch (NullPointerException e) {
            show(e);
        }
        try {
            System.out.println(a.objects.clone().length);
        } catch (NullPointerException e) {
            show(e);
        }
        a.groups = new int[] {3, 4};
        a.names = new String[2][];
        System.out.println(a.groups.clone()[1] + " " + a.names.clone().length);
    }
}
