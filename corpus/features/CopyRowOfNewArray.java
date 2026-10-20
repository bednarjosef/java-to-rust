public class CopyRowOfNewArray {
    static class Cell { int row; Cell(int row) { this.row = row; } }
    public static void main(String[] args) {
        Cell[][] grid = new Cell[2][2];
        grid[0][0] = new Cell(4);
        Cell[][] copy = new Cell[2][];
        copy[0] = grid[0];
        System.out.println(copy[0][0].row);
        copy[1] = null;
        System.out.println(copy[1] == null);
        int[][] ints = new int[3][2];
        int[][] alias = new int[2][];
        alias[1] = ints[2];
        ints[2][1] = 7;
        System.out.println(alias[1][1]);
        Cell[][][] cube = new Cell[2][2][2];
        cube[1][0][1] = new Cell(9);
        Cell[][][] cubeCopy = new Cell[2][][];
        cubeCopy[1] = cube[1];
        System.out.println(cubeCopy[1][0][1].row + " " + (cubeCopy[0] == null));
        Cell[][] fresh = new Cell[1][3];
        System.out.println(fresh[0].length + " " + (fresh[0][2] == null));
    }
}
