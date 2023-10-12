import os
import re
import time

import docker
import sys
import subprocess
import requests
import json
from urllib.parse import urlparse

from github_api_client import GithubClient

try:
    import click
except ImportError:
    print("Please install click library: pip install click==8.0.3")
    sys.exit(1)

ERR_MSG_TPL = {
    "blocks": [
        {
            "type": "section",
            "text": {"type": "mrkdwn", "text": ""},
        },
        {"type": "divider"},
    ]
}

DOCKER_USER = os.environ.get("DHUBU")
DOCKER_PASSWORD = os.environ.get("DHUBP")
IMAGE_NAME = 'neonlabsorg/evm_loader'
SOLANA_NODE_VERSION = 'v1.16.16'
SOLANA_BPF_VERSION = 'v1.16.16'

VERSION_BRANCH_TEMPLATE = r"[vt]{1}\d{1,2}\.\d{1,2}\.x.*"
docker_client = docker.APIClient()


@click.group()
def cli():
    pass


@cli.command(name="build_docker_image")
@click.option('--github_sha')
def build_docker_image(github_sha):
    solana_image = f'solanalabs/solana:{SOLANA_NODE_VERSION}'
    docker_client.pull(solana_image)
    buildargs = {"REVISION": github_sha,
                 "SOLANA_IMAGE": solana_image,
                 "SOLANA_BPF_VERSION": SOLANA_BPF_VERSION}

    tag = f"{IMAGE_NAME}:{github_sha}"
    click.echo("start build")
    output = docker_client.build(tag=tag, buildargs=buildargs, path="./", decode=True)
    process_output(output)


@cli.command(name="publish_image")
@click.option('--github_sha')
def publish_image(github_sha):
    docker_client.login(username=DOCKER_USER, password=DOCKER_PASSWORD)
    out = docker_client.push(f"{IMAGE_NAME}:{github_sha}", decode=True, stream=True)
    process_output(out)


@cli.command(name="finalize_image")
@click.option('--head_ref_branch')
@click.option('--github_ref')
@click.option('--github_sha')
def finalize_image(head_ref_branch, github_ref, github_sha):
    branch = github_ref.replace("refs/heads/", "")
    if re.match(VERSION_BRANCH_TEMPLATE, branch) is None:
        if 'refs/tags/' in branch:
            tag = branch.replace("refs/tags/", "")
        elif branch == 'master':
            tag = 'stable'
        elif branch == 'develop':
            tag = 'latest'
        else:
            tag = head_ref_branch.split('/')[-1]

        docker_client.login(username=DOCKER_USER, password=DOCKER_PASSWORD)
        out = docker_client.pull(f"{IMAGE_NAME}:{github_sha}", decode=True, stream=True)
        process_output(out)

        docker_client.tag(f"{IMAGE_NAME}:{github_sha}", f"{IMAGE_NAME}:{tag}")
        out = docker_client.push(f"{IMAGE_NAME}:{tag}", decode=True, stream=True)
        process_output(out)
    else:
        click.echo("The image is not published, please create tag for publishing")


def run_subprocess(command):
    click.echo(f"run command: {command}")
    subprocess.run(command, shell=True)


@cli.command(name="run_tests")
@click.option('--github_sha')
def run_tests(github_sha):
    image_name = f"{IMAGE_NAME}:{github_sha}"
    os.environ["EVM_LOADER_IMAGE"] = image_name
    project_name = f"neon-evm-{github_sha}"
    stop_containers(project_name)

    run_subprocess(f"docker-compose -p {project_name} -f ./ci/docker-compose-ci.yml up -d")
    container_name = get_solana_container_name(project_name)
    click.echo("Start tests")
    exec_id = docker_client.exec_create(
        container=container_name, cmd="/opt/deploy-test.sh")
    logs = docker_client.exec_start(exec_id['Id'], stream=True)

    tests_are_failed = False
    all_logs = ""
    for line in logs:
        current_line = line.decode('utf-8')
        all_logs += current_line
        click.echo(current_line)
        if 'ERROR ' in current_line or 'FAILED ' in current_line:
            tests_are_failed = True
            print("Tests are failed")
    if "[100%]" not in all_logs:
        tests_are_failed = True
        print("Part of tests are skipped")

    exec_status = docker_client.exec_inspect(exec_id['Id'])["ExitCode"]

    run_subprocess(f"docker-compose -p {project_name} -f ./ci/docker-compose-ci.yml logs dk-neon-api")

    stop_containers(project_name)

    if tests_are_failed or exec_status == 1:
        sys.exit(1)


def get_solana_container_name(project_name):
    data = subprocess.run(
        f"docker-compose -p {project_name} -f ./ci/docker-compose-ci.yml ps",
        shell=True, capture_output=True, text=True).stdout
    click.echo(data)
    pattern = rf'{project_name}_solana_[1-9]+'

    match = re.search(pattern, data)
    return match.group(0)


def stop_containers(project_name):
    run_subprocess(f"docker-compose -p {project_name} -f ./ci/docker-compose-ci.yml down")


@cli.command(name="trigger_proxy_action")
@click.option('--head_ref_branch')
@click.option('--base_ref_branch')
@click.option('--github_ref')
@click.option('--github_sha')
@click.option('--token')
@click.option('--is_draft')
@click.option('--labels')
@click.option('--pr_url')
@click.option('--pr_number')
def trigger_proxy_action(head_ref_branch, base_ref_branch, github_ref, github_sha, token, is_draft, labels,
                         pr_url, pr_number):
    is_develop_branch = github_ref in ['refs/heads/develop', 'refs/heads/master']
    is_tag_creating = 'refs/tags/' in github_ref
    is_version_branch = re.match(VERSION_BRANCH_TEMPLATE, github_ref.replace("refs/heads/", "")) is not None
    is_FTS_labeled_not_draft = 'fullTestSuit' in labels and is_draft != "true"
    is_extended_FTS_labeled_not_draft = 'extendedFullTestSuit' in labels and is_draft != "true"

    if is_extended_FTS_labeled_not_draft:
        test_set = "extendedFullTestSuite"
    elif is_develop_branch or is_tag_creating or is_version_branch or is_FTS_labeled_not_draft:
        test_set = "fullTestSuite"
    else:
        test_set = "basic"

    github = GithubClient(token)

    if head_ref_branch in github.get_proxy_branches():
        proxy_branch = head_ref_branch
    elif re.match(VERSION_BRANCH_TEMPLATE, base_ref_branch):
        proxy_branch = base_ref_branch
    elif is_tag_creating:
        neon_evm_tag = github_ref.replace("refs/tags/", "")
        proxy_branch = re.sub(r'\.\d+$', '.x', neon_evm_tag)
    elif is_version_branch:
        proxy_branch = github_ref.replace("refs/heads/", "")
    else:
        proxy_branch = 'develop'
    click.echo(f"Proxy branch: {proxy_branch}")

    initial_pr = f"{pr_url}/{pr_number}/comments" if pr_number else ""

    runs_before = github.get_proxy_runs_list(proxy_branch)
    runs_count_before = github.get_proxy_runs_count(proxy_branch)
    github.run_proxy_dispatches(proxy_branch, github_ref, github_sha, test_set, initial_pr)
    wait_condition(lambda: github.get_proxy_runs_count(proxy_branch) > runs_count_before)

    runs_after = github.get_proxy_runs_list(proxy_branch)
    proxy_run_id = list(set(runs_after) - set(runs_before))[0]
    link = f"https://github.com/neonlabsorg/proxy-model.py/actions/runs/{proxy_run_id}"
    click.echo(f"Proxy run link: {link}")
    click.echo("Waiting for completed status...")
    wait_condition(lambda: github.get_proxy_run_info(proxy_run_id)["status"] == "completed", timeout_sec=10800, delay=5)

    if github.get_proxy_run_info(proxy_run_id)["conclusion"] == "success":
        click.echo("Proxy tests passed successfully")
    else:
        raise RuntimeError(f"Proxy tests failed! See {link}")


def wait_condition(func_cond, timeout_sec=60, delay=0.5):
    start_time = time.time()
    while True:
        if time.time() - start_time > timeout_sec:
            raise RuntimeError(f"The condition not reached within {timeout_sec} sec")
        try:
            if func_cond():
                break
        except:
            raise
        time.sleep(delay)


@cli.command(name="send_notification", help="Send notification to slack")
@click.option("-u", "--url", help="slack app endpoint url.")
@click.option("-b", "--build_url", help="github action test build url.")
def send_notification(url, build_url):
    tpl = ERR_MSG_TPL.copy()

    parsed_build_url = urlparse(build_url).path.split("/")
    build_id = parsed_build_url[-1]
    repo_name = f"{parsed_build_url[1]}/{parsed_build_url[2]}"

    tpl["blocks"][0]["text"]["text"] = (
        f"*Build <{build_url}|`{build_id}`> of repository `{repo_name}` is failed.*"
        f"\n<{build_url}|View build details>"
    )
    requests.post(url=url, data=json.dumps(tpl))


def process_output(output):
    for line in output:
        if line:
            errors = set()
            try:
                if "status" in line:
                    click.echo(line["status"])

                elif "stream" in line:
                    stream = re.sub("^\n", "", line["stream"])
                    stream = re.sub("\n$", "", stream)
                    stream = re.sub("\n(\x1B\[0m)$", "\\1", stream)
                    if stream:
                        click.echo(stream)

                elif "aux" in line:
                    if "Digest" in line["aux"]:
                        click.echo("digest: {}".format(line["aux"]["Digest"]))

                    if "ID" in line["aux"]:
                        click.echo("ID: {}".format(line["aux"]["ID"]))

                else:
                    click.echo("not recognized (1): {}".format(line))

                if "error" in line:
                    errors.add(line["error"])

                if "errorDetail" in line:
                    errors.add(line["errorDetail"]["message"])

                    if "code" in line:
                        error_code = line["errorDetail"]["code"]
                        errors.add("Error code: {}".format(error_code))

            except ValueError as e:
                click.echo("not recognized (2): {}".format(line))

            if errors:
                message = "problem executing Docker: {}".format(". ".join(errors))
                raise SystemError(message)


if __name__ == "__main__":
    cli()
