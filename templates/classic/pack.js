import tar from "tar-fs";
import { createWriteStream, mkdirSync } from "fs";

const DIR = "../../src/templates";
mkdirSync(DIR, { recursive: true });

let out = createWriteStream(`${DIR}/classic.tar`);

tar.pack("./dist").pipe(out);
