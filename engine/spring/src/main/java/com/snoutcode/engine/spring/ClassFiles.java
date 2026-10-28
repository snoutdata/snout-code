package com.snoutcode.engine.spring;

import java.io.ByteArrayInputStream;
import java.io.DataInputStream;
import java.io.IOException;

/** Reads the {@code SourceFile} attribute of a class file, which says which Scala file a bridge class came from. */
final class ClassFiles {
    private ClassFiles() {
    }

    static String sourceFile(byte[] bytes) {
        try (DataInputStream in = new DataInputStream(new ByteArrayInputStream(bytes))) {
            if (in.readInt() != 0xCAFEBABE) {
                return null;
            }
            in.readUnsignedShort();
            in.readUnsignedShort();
            int count = in.readUnsignedShort();
            String[] utf8 = new String[count];
            for (int i = 1; i < count; i++) {
                int tag = in.readUnsignedByte();
                switch (tag) {
                    case 1 -> utf8[i] = in.readUTF();
                    case 3, 4 -> in.readInt();
                    case 5, 6 -> {
                        in.readLong();
                        i++;
                    }
                    case 7, 8, 16, 19, 20 -> in.readUnsignedShort();
                    case 9, 10, 11, 12, 17, 18 -> in.readInt();
                    case 15 -> {
                        in.readUnsignedByte();
                        in.readUnsignedShort();
                    }
                    default -> {
                        return null;
                    }
                }
            }
            in.readUnsignedShort();
            in.readUnsignedShort();
            in.readUnsignedShort();
            int interfaces = in.readUnsignedShort();
            in.skipNBytes(2L * interfaces);
            for (int kind = 0; kind < 2; kind++) {
                int members = in.readUnsignedShort();
                for (int m = 0; m < members; m++) {
                    in.skipNBytes(6);
                    skipAttributes(in);
                }
            }
            int attributes = in.readUnsignedShort();
            for (int a = 0; a < attributes; a++) {
                String name = utf8[in.readUnsignedShort()];
                int length = in.readInt();
                if ("SourceFile".equals(name)) {
                    return utf8[in.readUnsignedShort()];
                }
                in.skipNBytes(length);
            }
        } catch (IOException | RuntimeException e) {
            return null;
        }
        return null;
    }

    private static void skipAttributes(DataInputStream in) throws IOException {
        int attributes = in.readUnsignedShort();
        for (int a = 0; a < attributes; a++) {
            in.readUnsignedShort();
            in.skipNBytes(in.readInt() & 0xFFFFFFFFL);
        }
    }
}
