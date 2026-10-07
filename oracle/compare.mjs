// Runs upstream's CLI `validate` and this CLI on every case at each version both accept,
// with both output formats, and compares the outputs without what is meant to differ:
// the file path, versions and times, and the `code` and `keyword` keys this CLI adds.
import fs from "node:fs";
import path from "node:path";
import util from "node:util";
import { execFileSync } from "node:child_process";

const [upstream, ours, cases] = process.argv.slice(2);
const { validate } = await import(path.resolve(upstream, "dist/commands.js"));
const files = fs.readdirSync(cases, { recursive: true })
  .filter((f) => /\.(csv|json)$/.test(f))
  .map((f) => path.join(cases, f));
// The table's originalError cell is the JSON parser's stack trace upstream, `{}` here.
Error.prototype[util.inspect.custom] = () => "{}";

// Upstream prints with console.log and console.table; collect what they write.
async function upstreamOutput(file, version, output) {
  let text = "";
  const write = process.stdout.write;
  process.stdout.write = (s) => { text += s; return true; };
  try {
    await validate(file, version, { output, errorLimit: 1000 });
  } finally {
    process.stdout.write = write;
  }
  return text;
}

function ourOutput(file, version, output) {
  try {
    return execFileSync(ours, [file, version, "-o", output], { encoding: "utf-8" });
  } catch (e) {
    return e.stdout; // an invalid file exits with 1
  }
}

const comparable = {
  json(text) {
    const result = JSON.parse(text);
    for (const key of ["file", "validatorVersion", "timestamp"]) delete result[key];
    for (const finding of [...result.errors, ...result.alerts]) {
      delete finding.code;
      delete finding.keyword;
    }
    return JSON.stringify(result, null, 2);
  },
  table(text) {
    return text.split("\n").filter((line) => !/^(Validating file|Using |Validator run )/.test(line)).join("\n");
  },
};

let checked = 0, differ = 0;
for (const file of files) {
  for (const version of ["2.1.0", "2.2.0", "3.0.0"]) {
    for (const output of ["json", "table"]) {
      const theirs = comparable[output](await upstreamOutput(file, version, output));
      const mine = comparable[output](ourOutput(file, version, output));
      checked++;
      if (theirs === mine) continue;
      differ++;
      console.log(`differs: ${path.relative(cases, file)} ${version} ${output}`);
    }
  }
}
console.log(`${checked} outputs compared, ${differ} differ`);
process.exitCode = differ ? 1 : 0;
