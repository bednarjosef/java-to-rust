public class CubeObjects {
    static class Cell { int row; Cell(int row) { this.row = row; } }
    public static void main(String[] args) {
        Cell[][][] cube = new Cell[2][2][];
        cube[1][0] = new Cell[] { new Cell(3), null };
        System.out.println(cube[1][0][0].row + " " + cube[1][0].length);
    }
}
