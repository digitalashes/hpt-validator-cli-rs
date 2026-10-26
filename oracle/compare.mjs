// Runs upstream's CLI `validate` and this CLI on every case at each version both accept,
// with both output formats, and compares the outputs without what is meant to differ:
// the file path, versions and times, and the `code` and `keyword` keys this CLI adds.
// Then runs both CLIs on argument lists and compares the first line of standard error.
import fs from "node:fs";
import path from "node:path";
import util from "node:util";
import { execFileSync, spawnSync } from "node:child_process";

const [upstream, ours, cases] = process.argv.slice(2);
const { validate } = await import(path.resolve(upstream, "dist/commands.js"));
const files = fs.readdirSync(cases, { recursive: true })
  .filter((f) => /\.(csv|json)$/.test(f))
  .map((f) => path.join(cases, f));
// The table's originalError cell is the JSON parser's stack trace upstream, `{}` here.
Error.prototype[util.inspect.custom] = () => "{}";

// Upstream prints with console.log and console.table; collect what they write.
async function upstreamOutput(file, version, output, errorLimit) {
  let text = "";
  const write = process.stdout.write;
  process.stdout.write = (s) => { text += s; return true; };
  try {
    await validate(file, version, { output, errorLimit });
  } finally {
    process.stdout.write = write;
  }
  return text;
}

function ourOutput(file, version, output, errorLimit) {
  const args = [file, version, "-o", output, "-e", String(errorLimit)];
  try {
    return execFileSync(ours, args, { encoding: "utf-8", maxBuffer: 1 << 30 });
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
    // Upstream prints local time, this CLI UTC; the version lines name each validator.
    return text
      .replace(/^(Validator run \w+ at ).*$/gm, "$1")
      .replace(/^Using \S*hpt-validator version .*$/gm, "Using hpt-validator version");
  },
};

let checked = 0, differ = 0;
async function compare(file, version, output, errorLimit = 1000) {
  const theirs = comparable[output](await upstreamOutput(file, version, output, errorLimit));
  const mine = comparable[output](ourOutput(file, version, output, errorLimit));
  checked++;
  if (theirs === mine) return;
  differ++;
  console.log(`differs: ${path.relative(cases, file)} ${version} ${output} -e ${errorLimit}`);
}
for (const file of files) {
  for (const version of ["2.1.0", "2.2.0", "3.0.0"]) {
    for (const output of ["json", "table"]) await compare(file, version, output);
  }
}
// Upstream prints errors as one table per 1000. Every code number up to the count that row 3
// lacks is two errors, so `code | 2000` fills the limit.
const many = "target/many-errors.csv";
const few = fs.readFileSync(path.join(cases, "findings/code-count-past-columns.csv"), "utf-8");
fs.writeFileSync(many, few.replace("code | 40", "code | 2000"));
await compare(many, "2.2.0", "table", 2500);
// Argument lists, each run after a case file; most are refused.
const file = files[0];
const argLists = [
  [], ["3", "extra"], ["v3"], ["3.0"], ["v3.0.0"], ["2.0"], ["03"], ["2.01"], ["3.0.01"],
  ["x3"], ["3x"], ["3x5"], ["3."], [".3"], ["v3.0.0-beta"], ["3.0.0.1"], ["3.0.0.01"],
  ["3.0.9007199254740991"], ["3.0.9007199254740992"], ["99999999999999999"],
  ["abc"],
  ["3", "-e", "x"], ["3", "-e=5"], ["3", "-e", " 7"], ["3", "-e", "1e3"], ["3", "--x"],
  ["3", "-x5"], ["3", "-é"], ["3", "-o", "xml"], ["3", "-oxml"], ["3", "-f", "txt"],
  ["3", "--output=xml"], ["3", "--error-limit"], ["3", "-d5"], ["3", "-d=5"], ["3", "-dx"],
  ["3", "-de", "x"], ["3", "--debug=x"],
];
const firstLine = (cmd, args) =>
  spawnSync(cmd, args, { encoding: "utf-8" }).stderr.split("\n")[0];
for (const args of argLists) {
  const theirs = firstLine("node", [path.join(upstream, "dist/index.js"), file, ...args]);
  const mine = firstLine(ours, [file, ...args]);
  checked++;
  if (theirs === mine) continue;
  differ++;
  console.log(`differs: ${args.join(" ")}\n  theirs ${theirs}\n  mine   ${mine}`);
}
console.log(`${checked} outputs compared, ${differ} differ`);
process.exitCode = differ ? 1 : 0;
