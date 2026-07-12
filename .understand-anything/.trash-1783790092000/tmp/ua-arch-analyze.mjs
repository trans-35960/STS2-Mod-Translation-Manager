import fs from "node:fs";
import path from "node:path";

const [inputPath, outputPath] = process.argv.slice(2);
if (!inputPath || !outputPath) {
  console.error("usage: node ua-arch-analyze.mjs <input> <output>");
  process.exit(1);
}

try {
  const input = JSON.parse(fs.readFileSync(inputPath, "utf8"));
  const { fileNodes, importEdges, allEdges } = input;
  if (!Array.isArray(fileNodes) || !Array.isArray(importEdges) || !Array.isArray(allEdges)) {
    throw new Error("input must contain fileNodes, importEdges, and allEdges arrays");
  }

  const byId = new Map(fileNodes.map((node) => [node.id, node]));
  if (byId.size !== fileNodes.length) throw new Error("duplicate file node IDs");
  const paths = fileNodes.map((node) => node.filePath.replaceAll("\\", "/"));
  const pathSegments = paths.map((value) => value.split("/").filter(Boolean));
  const common = pathSegments.length ? [...pathSegments[0]] : [];
  while (common.length && pathSegments.some((segments) => segments[common.length - 1] !== common[common.length - 1])) common.pop();
  if (common.length && pathSegments.some((segments) => segments.length === common.length)) common.pop();
  const commonPathPrefix = common.length ? `${common.join("/")}/` : "";

  const rawGroups = new Map();
  const directoryGroupByNode = {};
  for (const node of fileNodes) {
    const normalized = node.filePath.replaceAll("\\", "/");
    const relative = commonPathPrefix && normalized.startsWith(commonPathPrefix) ? normalized.slice(commonPathPrefix.length) : normalized;
    const segments = relative.split("/").filter(Boolean);
    let group = segments.length > 1 ? segments[0] : "root";
    if (paths.every((value) => !value.includes("/"))) {
      if (/\.(?:test|spec)\./.test(relative)) group = "test";
      else if (/config|\.json$|\.toml$|\.ya?ml$/.test(relative)) group = "config";
      else group = path.extname(relative).slice(1) || "root";
    }
    directoryGroupByNode[node.id] = group;
    if (!rawGroups.has(group)) rawGroups.set(group, []);
    rawGroups.get(group).push(node.id);
  }
  const directoryGroups = Object.fromEntries([...rawGroups.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([group, ids]) => [group, ids.sort()]));

  const nodeTypeGroups = {};
  for (const node of fileNodes) (nodeTypeGroups[node.type] ??= []).push(node.id);
  for (const ids of Object.values(nodeTypeGroups)) ids.sort();

  const fileFanIn = Object.fromEntries(fileNodes.map((node) => [node.id, 0]));
  const fileFanOut = Object.fromEntries(fileNodes.map((node) => [node.id, 0]));
  const importAdjacency = Object.fromEntries(fileNodes.map((node) => [node.id, []]));
  const validImports = importEdges.filter((edge) => byId.has(edge.source) && byId.has(edge.target));
  for (const edge of validImports) {
    fileFanOut[edge.source] += 1;
    fileFanIn[edge.target] += 1;
    importAdjacency[edge.source].push(edge.target);
  }
  for (const values of Object.values(importAdjacency)) values.sort();

  const groupDependencySets = Object.fromEntries(Object.keys(directoryGroups).map((group) => [group, { importsFrom: new Set(), importedBy: new Set() }]));
  const interCounts = new Map();
  for (const edge of validImports) {
    const from = directoryGroupByNode[edge.source];
    const to = directoryGroupByNode[edge.target];
    if (from !== to) {
      groupDependencySets[from].importsFrom.add(to);
      groupDependencySets[to].importedBy.add(from);
      const key = `${from}\u0000${to}`;
      interCounts.set(key, (interCounts.get(key) ?? 0) + 1);
    }
  }
  const groupDependencies = Object.fromEntries(Object.entries(groupDependencySets).map(([group, sets]) => [group, { importsFrom: [...sets.importsFrom].sort(), importedBy: [...sets.importedBy].sort() }]));
  const interGroupImports = [...interCounts.entries()].map(([key, count]) => { const [from, to] = key.split("\u0000"); return { from, to, count }; }).sort((a, b) => b.count - a.count || a.from.localeCompare(b.from) || a.to.localeCompare(b.to));

  const intraGroupDensity = {};
  for (const group of Object.keys(directoryGroups)) {
    let internalEdges = 0;
    let totalEdges = 0;
    for (const edge of validImports) {
      const from = directoryGroupByNode[edge.source];
      const to = directoryGroupByNode[edge.target];
      if (from === group || to === group) totalEdges += 1;
      if (from === group && to === group) internalEdges += 1;
    }
    intraGroupDensity[group] = { internalEdges, totalEdges, density: totalEdges ? Number((internalEdges / totalEdges).toFixed(4)) : 0 };
  }

  const fileLevelEdges = allEdges.filter((edge) => byId.has(edge.source) && byId.has(edge.target));
  const crossCounts = new Map();
  for (const edge of fileLevelEdges) {
    const fromType = byId.get(edge.source).type;
    const toType = byId.get(edge.target).type;
    const key = `${fromType}\u0000${toType}\u0000${edge.type}`;
    crossCounts.set(key, (crossCounts.get(key) ?? 0) + 1);
  }
  const crossCategoryEdges = [...crossCounts.entries()].map(([key, count]) => { const [fromType, toType, edgeType] = key.split("\u0000"); return { fromType, toType, edgeType, count }; }).sort((a, b) => b.count - a.count || a.fromType.localeCompare(b.fromType));
  const nonCodeConnections = fileLevelEdges.filter((edge) => byId.get(edge.source).type !== "file" || byId.get(edge.target).type !== "file").map((edge) => ({ source: edge.source, target: edge.target, type: edge.type }));

  const directoryPatterns = [
    [/^(routes|api|controllers|endpoints|handlers|serializers|controller|routers|blueprints)$/i, "api"],
    [/^(services|core|lib|domain|logic|signals|internal|mailers|jobs|channels|src)$/i, "service"],
    [/^(models|db|data|persistence|repository|entities|migrations|sql|database|schema|entity)$/i, "data"],
    [/^(components|views|pages|ui|layouts|screens)$/i, "ui"],
    [/^(middleware|plugins|interceptors|guards)$/i, "middleware"],
    [/^(utils|helpers|common|shared|tools|templatetags|pkg|scripts)$/i, "utility"],
    [/^(config|constants|env|settings|management|commands|request|response|dto)$/i, "config"],
    [/^(__tests__|test|tests|spec|specs)$/i, "test"],
    [/^(types|interfaces|schemas|contracts|dtos)$/i, "types"],
    [/^hooks$/i, "hooks"],
    [/^(store|state|reducers|actions|slices)$/i, "state"],
    [/^(assets|static|public)$/i, "assets"],
    [/^(docs|documentation|wiki)$/i, "documentation"],
    [/^(deploy|deployment|infra|infrastructure|k8s|kubernetes|helm|charts|terraform|tf|docker)$/i, "infrastructure"],
    [/^(\.github|\.gitlab|\.circleci)$/i, "ci-cd"],
    [/^(cmd|bin)$/i, "entry"],
  ];
  const patternMatches = {};
  for (const group of Object.keys(directoryGroups)) {
    const match = directoryPatterns.find(([regex]) => regex.test(group));
    if (match) patternMatches[group] = match[1];
  }
  const classifyFile = (filePath) => {
    const normalized = filePath.replaceAll("\\", "/");
    const name = path.posix.basename(normalized);
    if (/\.(?:test|spec)\.[^.]+$/.test(name) || /^test_.*\.py$/.test(name) || /_test\.go$/.test(name)) return "test";
    if (/\.d\.ts$/.test(name)) return "types";
    if ((name === "main.rs" || name === "lib.rs") && /(^|\/)src\//.test(normalized)) return "entry";
    if (["Cargo.toml", "go.mod", "Gemfile", "pom.xml", "build.gradle", "composer.json"].includes(name)) return "config";
    if (/^(Dockerfile(?:\..*)?|docker-compose\..*)$/.test(name) || /\.tf(?:vars)?$/.test(name)) return "infrastructure";
    if (/^\.github\/workflows\//.test(normalized) || name === ".gitlab-ci.yml" || name === "Jenkinsfile") return "ci-cd";
    if (/\.sql$/.test(name)) return "data";
    if (/\.(?:graphql|gql|proto)$/.test(name)) return "types";
    if (/\.(?:md|rst)$/.test(name)) return "documentation";
    if (name === "Makefile") return "infrastructure";
    if (/^(index\.(?:ts|js)|__init__\.py)$/.test(name)) return "entry";
    return null;
  };
  const filePatternMatches = Object.fromEntries(fileNodes.map((node) => [node.id, classifyFile(node.filePath)]).filter(([, match]) => match));

  const normalizedPaths = fileNodes.map((node) => node.filePath.replaceAll("\\", "/"));
  const infraFiles = normalizedPaths.filter((value) => /(^|\/)(Dockerfile(?:\..*)?|docker-compose\.ya?ml|compose\.ya?ml|Jenkinsfile|\.gitlab-ci\.yml)$/.test(value) || /(^|\/)\.github\/workflows\/.*\.ya?ml$/.test(value) || /\.(?:tf|tfvars)$/.test(value) || /(^|\/)(?:k8s|kubernetes|helm|charts)\//.test(value));
  const deploymentTopology = {
    hasDockerfile: infraFiles.some((value) => /(^|\/)Dockerfile(?:\..*)?$/.test(value)),
    hasCompose: infraFiles.some((value) => /(^|\/)(?:docker-compose|compose)\.ya?ml$/.test(value)),
    hasK8s: infraFiles.some((value) => /(^|\/)(?:k8s|kubernetes|helm|charts)\//.test(value)),
    hasTerraform: infraFiles.some((value) => /\.(?:tf|tfvars)$/.test(value)),
    hasCI: infraFiles.some((value) => /(^|\/)(?:\.github\/workflows|\.circleci)\//.test(value) || /(^|\/)(?:\.gitlab-ci\.yml|Jenkinsfile)$/.test(value)),
    infraFiles: infraFiles.sort(),
  };

  const dataPipeline = {
    schemaFiles: fileNodes.filter((node) => node.type === "schema" || /\.(?:graphql|gql|proto|prisma)$/.test(node.filePath)).map((node) => node.filePath).sort(),
    migrationFiles: fileNodes.filter((node) => /(^|\/)migrations?\//.test(node.filePath) || /\.sql$/.test(node.filePath)).map((node) => node.filePath).sort(),
    dataModelFiles: fileNodes.filter((node) => node.tags?.some((tag) => ["data-model", "데이터모델", "entity"].includes(tag)) || /(^|\/)(?:models|entities|dto)\//.test(node.filePath)).map((node) => node.filePath).sort(),
    apiHandlerFiles: fileNodes.filter((node) => node.tags?.some((tag) => ["api-handler", "controller", "endpoint", "tauri-command"].includes(tag)) || /(^|\/)(?:routes|controllers|handlers|api)\//.test(node.filePath)).map((node) => node.filePath).sort(),
  };

  const groupsWithDocs = new Set();
  for (const node of fileNodes.filter((node) => node.type === "document")) groupsWithDocs.add(directoryGroupByNode[node.id]);
  for (const edge of fileLevelEdges.filter((edge) => edge.type === "documents")) {
    if (byId.get(edge.source)?.type === "document") groupsWithDocs.add(directoryGroupByNode[edge.target]);
  }
  const allGroups = Object.keys(directoryGroups);
  const docCoverage = {
    groupsWithDocs: groupsWithDocs.size,
    totalGroups: allGroups.length,
    coverageRatio: allGroups.length ? Number((groupsWithDocs.size / allGroups.length).toFixed(4)) : 0,
    undocumentedGroups: allGroups.filter((group) => !groupsWithDocs.has(group)).sort(),
  };

  const dependencyDirection = [];
  const handledPairs = new Set();
  for (const { from, to } of interGroupImports) {
    const pair = [from, to].sort().join("\u0000");
    if (handledPairs.has(pair)) continue;
    handledPairs.add(pair);
    const forward = interCounts.get(`${from}\u0000${to}`) ?? 0;
    const reverse = interCounts.get(`${to}\u0000${from}`) ?? 0;
    if (forward > reverse) dependencyDirection.push({ dependent: from, dependsOn: to, forwardCount: forward, reverseCount: reverse });
    else if (reverse > forward) dependencyDirection.push({ dependent: to, dependsOn: from, forwardCount: reverse, reverseCount: forward });
  }
  dependencyDirection.sort((a, b) => b.forwardCount - a.forwardCount || a.dependent.localeCompare(b.dependent));

  const fileInfo = Object.fromEntries(fileNodes.map((node) => [node.id, { filePath: node.filePath, type: node.type, summary: node.summary, tags: node.tags }]));
  const result = {
    scriptCompleted: true,
    commonPathPrefix,
    directoryGroups,
    directoryGroupByNode,
    nodeTypeGroups,
    importAdjacency,
    groupDependencies,
    crossCategoryEdges,
    nonCodeConnections,
    interGroupImports,
    intraGroupDensity,
    patternMatches,
    filePatternMatches,
    deploymentTopology,
    dataPipeline,
    docCoverage,
    dependencyDirection,
    fileStats: {
      totalFileNodes: fileNodes.length,
      filesPerGroup: Object.fromEntries(Object.entries(directoryGroups).map(([group, ids]) => [group, ids.length])),
      nodeTypeCounts: Object.fromEntries(Object.entries(nodeTypeGroups).map(([type, ids]) => [type, ids.length])),
      importEdgeCount: validImports.length,
      inputAllEdgeCount: allEdges.length,
      fileLevelAllEdgeCount: fileLevelEdges.length,
    },
    fileFanIn,
    fileFanOut,
    fileInfo,
  };
  fs.writeFileSync(outputPath, `${JSON.stringify(result, null, 2)}\n`);
  console.error(`architecture-analysis: files=${fileNodes.length} imports=${validImports.length} groups=${Object.keys(directoryGroups).length} fileLevelEdges=${fileLevelEdges.length}`);
} catch (error) {
  console.error(error instanceof Error ? error.stack : String(error));
  process.exit(1);
}
