import type { Bucket } from "$lib/api/resource-groups";
import type { CodeLanguage } from "./code-highlight";

export interface BucketSnippet {
    id: string;
    label: string;
    filename: string;
    tag: string;
    language: CodeLanguage;
    code: string;
}

const ACCESS_KEY_PLACEHOLDER = "<access key id>";
const SECRET_KEY_PLACEHOLDER = "<secret access key>";
const REGION = "garage";

export function bucketEndpoint(bucket: Bucket, origin: string): string {
    return bucket.exposure === "external"
        ? `${origin}/api/s3/${bucket.global_alias}`
        : `http://s3.svc.${bucket.resource_group_id}.internal:3900`;
}

function awsCli(alias: string, endpoint: string): string {
    return [
        `export AWS_ACCESS_KEY_ID=${ACCESS_KEY_PLACEHOLDER}`,
        `export AWS_SECRET_ACCESS_KEY=${SECRET_KEY_PLACEHOLDER}`,
        `export AWS_DEFAULT_REGION=${REGION}`,
        "",
        "# list objects",
        `aws s3 ls s3://${alias} --endpoint-url ${endpoint}`,
        "",
        "# upload a file",
        `aws s3 cp ./file.txt s3://${alias}/file.txt --endpoint-url ${endpoint}`,
    ].join("\n");
}

function python(alias: string, endpoint: string): string {
    return [
        "import boto3",
        "from botocore.config import Config",
        "",
        "s3 = boto3.client(",
        '    "s3",',
        `    endpoint_url="${endpoint}",`,
        `    aws_access_key_id="${ACCESS_KEY_PLACEHOLDER}",`,
        `    aws_secret_access_key="${SECRET_KEY_PLACEHOLDER}",`,
        `    region_name="${REGION}",`,
        '    config=Config(s3={"addressing_style": "path"}),',
        ")",
        "",
        `for item in s3.list_objects_v2(Bucket="${alias}").get("Contents", []):`,
        '    print(item["Key"], item["Size"])',
    ].join("\n");
}

function javascript(alias: string, endpoint: string): string {
    return [
        'import { S3Client, ListObjectsV2Command } from "@aws-sdk/client-s3";',
        "",
        "const client = new S3Client({",
        `  endpoint: "${endpoint}",`,
        `  region: "${REGION}",`,
        "  forcePathStyle: true,",
        `  credentials: { accessKeyId: "${ACCESS_KEY_PLACEHOLDER}", secretAccessKey: "${SECRET_KEY_PLACEHOLDER}" },`,
        "});",
        "",
        `const { Contents = [] } = await client.send(new ListObjectsV2Command({ Bucket: "${alias}" }));`,
        "for (const item of Contents) console.log(item.Key, item.Size);",
    ].join("\n");
}

function rclone(endpoint: string): string {
    return [
        "[csfx]",
        "type = s3",
        "provider = Other",
        `access_key_id = ${ACCESS_KEY_PLACEHOLDER}`,
        `secret_access_key = ${SECRET_KEY_PLACEHOLDER}`,
        `endpoint = ${endpoint}`,
        `region = ${REGION}`,
        "force_path_style = true",
    ].join("\n");
}

export function bucketSnippets(bucket: Bucket, origin: string): BucketSnippet[] {
    const endpoint = bucketEndpoint(bucket, origin);
    const alias = bucket.global_alias;
    return [
        { id: "aws-cli", label: "AWS CLI", filename: "s3.sh", tag: "shell", language: "shell", code: awsCli(alias, endpoint) },
        { id: "python", label: "Python", filename: "s3.py", tag: "boto3", language: "python", code: python(alias, endpoint) },
        { id: "javascript", label: "JavaScript", filename: "s3.mjs", tag: "aws-sdk", language: "javascript", code: javascript(alias, endpoint) },
        { id: "rclone", label: "rclone", filename: "rclone.conf", tag: "rclone", language: "ini", code: rclone(endpoint) },
    ];
}
