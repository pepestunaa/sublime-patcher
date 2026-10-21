pub const BUILD: &str = "4200";

pub struct Patch {
    pub name: &'static str,
    pub description: &'static str,
    pub original: &'static [u8],
    pub replacement: &'static [u8],
}

pub const PATCHES: &[Patch] = &[
    Patch {
        name: "UI/license flag",
        description: "movzx/xor flag sequence -> direct set (file offset 0x46B80).",
        // FILE OFFSET 0x46B80 (VA 0x140047780)
        // 0F B6 51 05   movzx edx, byte ptr [rcx+5]
        // 83 F2 01      xor   edx, 1
        original: &[0x0F, 0xB6, 0x51, 0x05, 0x83, 0xF2, 0x01],
        replacement: &[0xC6, 0x41, 0x05, 0x01, 0xB2, 0x00, 0x90],
    },
    Patch {
        name: "automatic update-check override",
        description: "JNZ -> JMP so the forced update_check write is skipped (file offset 0x32623).",
        // FILE OFFSET 0x3261F (VA 0x14003321F)
        // 80 78 05 00        cmp byte ptr [rax+5], 0
        // 75 07              jnz +7                 <- byte at 0x32623
        // C6 05 B4 DA 6C 00 01  mov byte ptr [0x140700CE0], 1
        original: &[
            0x80, 0x78, 0x05, 0x00, 0x75, 0x07, 0xC6, 0x05, 0xB4, 0xDA, 0x6C, 0x00, 0x01,
        ],
        replacement: &[
            0x80, 0x78, 0x05, 0x00, 0xEB, 0x07, 0xC6, 0x05, 0xB4, 0xDA, 0x6C, 0x00, 0x01,
        ],
    },
];
