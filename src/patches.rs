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
    Patch {
        name: "update-check startup gate",
        description: "JNZ -> JMP so update_check is loaded at startup even without a license (file offset 0x31B52).",
        // FILE OFFSET 0x31B47 (VA 0x140032747)
        // 48 8B 86 B0 02 00 00   mov rax, qword ptr [rsi+0x2b0]   ; license state
        // 80 78 05 01            cmp byte ptr [rax+5], 1
        // 75 30                  jnz +0x30                        <- byte at 0x31B52
        // Without this, an unlicensed copy never loads the user's update_check
        // setting at startup, so the auto-check flag keeps its .data initial
        // value of 1 and a due check still fires even with the override patch
        // above applied.
        original: &[
            0x48, 0x8B, 0x86, 0xB0, 0x02, 0x00, 0x00, 0x80, 0x78, 0x05, 0x01, 0x75, 0x30,
        ],
        replacement: &[
            0x48, 0x8B, 0x86, 0xB0, 0x02, 0x00, 0x00, 0x80, 0x78, 0x05, 0x01, 0xEB, 0x30,
        ],
    },
];
