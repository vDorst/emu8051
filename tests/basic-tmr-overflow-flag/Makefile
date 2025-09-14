CC = sdcc
ASM = sdas8051
AFLAGS= -plosgff
CFLAGS = --model-small --std-sdcc99 -mmcs51
OBJCOPY = objcopy

SRC = main.c
OBJ = $(patsubst %.c, output/%.rel, $(SRC))
TARGET_HEX = output/main.hex
TARGET_BIN = output/main.bin
TARGET_REL = output/crtstart.rel

all: create_output_dir $(TARGET_HEX) $(TARGET_BIN)

create_output_dir:
	mkdir -p output

$(TARGET_REL): crtstart.asm
	${ASM} ${AFLAGS} $(TARGET_REL) $^

$(TARGET_HEX): $(TARGET_REL) $(OBJ)
	$(CC) -o $(TARGET_HEX) -Wl-bHOME=0x100 $^

$(TARGET_BIN): $(TARGET_HEX)
	$(OBJCOPY) --input-target=ihex -O binary $(TARGET_HEX) $(TARGET_BIN)

output/%.rel: %.c
	$(CC) $(CFLAGS) $(LFLAGS) -c $< -o $@

clean:
	rm -rf output/
