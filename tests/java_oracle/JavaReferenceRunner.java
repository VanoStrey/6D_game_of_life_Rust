package org.example;

import java.io.FileWriter;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;

/**
 * Deterministic test vector generator using the original LogicGameOfLive.
 * Exports exact multi-step simulation states to JSON for Rust verification.
 */
public class JavaReferenceRunner {

    static class TestCase {
        public String name;
        public int size;
        public int dimensions;
        public double minPercent;
        public double maxPercent;
        public List<Integer> initial;
        public List<Integer> step1;
        public List<Integer> step2;
        public List<Integer> step3;
    }

    public static void main(String[] args) throws Exception {
        String outputFile = args.length > 0 ? args[0] : "tests/java_oracle/oracle_data.json";
        List<TestCase> testCases = new ArrayList<>();

        // Test suite 1: All dimensions 1D to 6D with size = 1
        for (int dim = 1; dim <= 6; dim++) {
            testCases.add(runCase("size1_dim" + dim, 1, dim, 20.0, 45.0, 42L + dim));
        }

        // Test suite 2: Dimensions 1D to 6D with size = 2
        for (int dim = 1; dim <= 6; dim++) {
            testCases.add(runCase("size2_dim" + dim, 2, dim, 20.0, 45.0, 100L + dim));
        }

        // Test suite 3: Dimensions 1D to 5D with size = 3
        for (int dim = 1; dim <= 5; dim++) {
            testCases.add(runCase("size3_dim" + dim, 3, dim, 20.0, 45.0, 200L + dim));
        }

        // Test suite 4: 6D with size = 3 (729 cells)
        testCases.add(runCase("size3_dim6", 3, 6, 20.0, 45.0, 300L));

        // Test suite 5: 4D with size = 4 (256 cells)
        testCases.add(runCase("size4_dim4", 4, 4, 20.0, 45.0, 400L));

        // Test suite 6: 4D with size = 6 (Default UI settings: 1296 cells)
        testCases.add(runCase("default_ui_4d_size6", 6, 4, 20.0, 45.0, 500L));

        // Serialize to JSON
        StringBuilder json = new StringBuilder();
        json.append("[\n");
        for (int i = 0; i < testCases.size(); i++) {
            TestCase tc = testCases.get(i);
            json.append("  {\n");
            json.append("    \"name\": \"").append(tc.name).append("\",\n");
            json.append("    \"size\": ").append(tc.size).append(",\n");
            json.append("    \"dimensions\": ").append(tc.dimensions).append(",\n");
            json.append("    \"minPercent\": ").append(tc.minPercent).append(",\n");
            json.append("    \"maxPercent\": ").append(tc.maxPercent).append(",\n");
            json.append("    \"initial\": ").append(tc.initial.toString()).append(",\n");
            json.append("    \"step1\": ").append(tc.step1.toString()).append(",\n");
            json.append("    \"step2\": ").append(tc.step2.toString()).append(",\n");
            json.append("    \"step3\": ").append(tc.step3.toString()).append("\n");
            json.append("  }");
            if (i < testCases.size() - 1) json.append(",");
            json.append("\n");
        }
        json.append("]\n");

        try (FileWriter writer = new FileWriter(outputFile)) {
            writer.write(json.toString());
        }
        System.out.println("Successfully generated " + testCases.size() + " Java test cases into " + outputFile);
    }

    private static TestCase runCase(String name, int size, int dimensions, double minPercent, double maxPercent, long seed) throws Exception {
        LogicGameOfLive logic = new LogicGameOfLive(size, dimensions, 3);

        Field boardField = LogicGameOfLive.class.getDeclaredField("gameBoard");
        boardField.setAccessible(true);
        boolean[][][][][][] board = (boolean[][][][][][]) boardField.get(logic);

        Field sizesField = LogicGameOfLive.class.getDeclaredField("sizeInDimensions");
        sizesField.setAccessible(true);
        int[] sizeInDimensions = (int[]) sizesField.get(logic);

        // Populate deterministically with LCG pseudo-random
        Random rng = new Random(seed);
        for (int a = 0; a < sizeInDimensions[5]; a++) {
            for (int b = 0; b < sizeInDimensions[4]; b++) {
                for (int c = 0; c < sizeInDimensions[3]; c++) {
                    for (int d = 0; d < sizeInDimensions[2]; d++) {
                        for (int e = 0; e < sizeInDimensions[1]; e++) {
                            for (int f = 0; f < sizeInDimensions[0]; f++) {
                                board[a][b][c][d][e][f] = rng.nextBoolean();
                            }
                        }
                    }
                }
            }
        }

        TestCase tc = new TestCase();
        tc.name = name;
        tc.size = size;
        tc.dimensions = dimensions;
        tc.minPercent = minPercent;
        tc.maxPercent = maxPercent;
        tc.initial = flattenBoard(board, sizeInDimensions);

        // Step 1
        logic.updateGameBoard(minPercent, maxPercent);
        board = (boolean[][][][][][]) boardField.get(logic);
        tc.step1 = flattenBoard(board, sizeInDimensions);

        // Step 2
        logic.updateGameBoard(minPercent, maxPercent);
        board = (boolean[][][][][][]) boardField.get(logic);
        tc.step2 = flattenBoard(board, sizeInDimensions);

        // Step 3
        logic.updateGameBoard(minPercent, maxPercent);
        board = (boolean[][][][][][]) boardField.get(logic);
        tc.step3 = flattenBoard(board, sizeInDimensions);

        return tc;
    }

    private static List<Integer> flattenBoard(boolean[][][][][][] board, int[] sizeInDimensions) {
        List<Integer> list = new ArrayList<>();
        // Row-major flatten matching linear_index:
        // f + e*S0 + d*S0*S1 + c*S0*S1*S2 + b*S0..S3 + a*S0..S4
        // Outer loop: a, then b, c, d, e, f
        for (int a = 0; a < sizeInDimensions[5]; a++) {
            for (int b = 0; b < sizeInDimensions[4]; b++) {
                for (int c = 0; c < sizeInDimensions[3]; c++) {
                    for (int d = 0; d < sizeInDimensions[2]; d++) {
                        for (int e = 0; e < sizeInDimensions[1]; e++) {
                            for (int f = 0; f < sizeInDimensions[0]; f++) {
                                list.add(board[a][b][c][d][e][f] ? 1 : 0);
                            }
                        }
                    }
                }
            }
        }
        return list;
    }
}
