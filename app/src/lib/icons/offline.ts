import { addCollection } from "@iconify/svelte";
import logos from "./logos.json";
import mdi from "./mdi.json";
import simpleIcons from "./simple-icons.json";

for (const collection of [logos, mdi, simpleIcons]) addCollection(collection);
