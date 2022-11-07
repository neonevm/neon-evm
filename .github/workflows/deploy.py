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
SOLANA_REVISION = 'v1.11.10'

VERSION_BRANCH_TEMPLATE = r"[vt]{1}\d{1,2}\.\d{1,2}\.x.*"
docker_client = docker.APIClient()


@click.group()
def cli():
    pass


@cli.command(name="build_docker_image")
@click.option('--github_sha')
def build_docker_image(github_sha):
    solana_image = f'solanalabs/solana:{SOLANA_REVISION}'
    docker_client.pull(solana_image)
    buildargs = {"REVISION": github_sha,
                 "SOLANA_IMAGE": solana_image,
                 "SOLANA_REVISION": SOLANA_REVISION}

    tag = f"{IMAGE_NAME}:{github_sha}"
    click.echo("start build")
    output = docker_client.build(tag=tag, buildargs=buildargs, path="./", decode=True)
    for line in output:
        if list(line.keys())[0] in ('stream', 'error', 'status'):
            value = list(line.values())[0].strip()
            if value:
                if "progress" in line.keys():
                    value += line['progress']
            click.echo(value)


@cli.command(name="publish_image")
@click.option('--github_sha')
def publish_image(github_sha):
    docker_client.login(username=DOCKER_USER, password=DOCKER_PASSWORD)
    out = docker_client.push(f"{IMAGE_NAME}:{github_sha}")
    if "error" in out:
        raise RuntimeError(
            f"Push {IMAGE_NAME}:{github_sha} finished with error: {out}")


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
        out = docker_client.pull(f"{IMAGE_NAME}:{github_sha}")
        if "error" in out:
            raise RuntimeError(
                f"Pull {IMAGE_NAME}:{github_sha} finished with error: {out}")

        docker_client.tag(f"{IMAGE_NAME}:{github_sha}", f"{IMAGE_NAME}:{tag}")
        out = docker_client.push(f"{IMAGE_NAME}:{tag}")
        if "error" in out:
            raise RuntimeError(
                f"Push {IMAGE_NAME}:{tag} finished with error: {out}")
        click.echo(f"The image {IMAGE_NAME}:{tag} is published")
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
    run_subprocess(f"docker-compose -f ./evm_loader/docker-compose-test.yml down")
    run_subprocess(f"docker-compose -f ./evm_loader/docker-compose-test.yml up -d")

    try:
        click.echo("Start tests")
        exec_id = docker_client.exec_create(
            container="solana", cmd="/opt/deploy-test.sh")
        logs = docker_client.exec_start(exec_id['Id'])
        click.echo(f'logs: {logs}')
        for line in logs:
            if 'ERROR ' in str(line) or 'FAILED ' in str(line):
                raise RuntimeError("Test are failed")
    except:
        raise RuntimeError("Solana container is not run")


@cli.command(name="stop_containers")
def stop_containers():
    run_subprocess(f"docker-compose -f ./evm_loader/docker-compose-test.yml down")


@cli.command(name="trigger_proxy_action")
@click.option('--head_ref_branch')
@click.option('--base_ref_branch')
@click.option('--github_ref')
@click.option('--github_sha')
@click.option('--token')
@click.option('--is_draft')
@click.option('--labels')
def trigger_proxy_action(head_ref_branch, base_ref_branch, github_ref, github_sha, token, is_draft, labels):
    is_develop_branch = github_ref in ['refs/heads/develop', 'refs/heads/master']
    is_tag_creating = 'refs/tags/' in github_ref
    is_version_branch = re.match(VERSION_BRANCH_TEMPLATE, github_ref.replace("refs/heads/", "")) is not None
    is_FTS_labeled_not_draft = 'FullTestSuit' in labels and is_draft != "true"

    if is_develop_branch or is_tag_creating or is_version_branch or is_FTS_labeled_not_draft:
        full_test_suite = "true"
    else:
        full_test_suite = "false"

    github = GithubClient(token)

    if head_ref_branch in github.get_proxy_branches():
        proxy_branch = head_ref_branch
    elif re.match(VERSION_BRANCH_TEMPLATE, base_ref_branch):
        proxy_branch = base_ref_branch
    elif is_tag_creating:
        proxy_branch = github_ref.replace("refs/tags/", "")
    elif is_version_branch:
        proxy_branch = github_ref.replace("refs/heads/", "")
    else:
        proxy_branch = 'develop'
    click.echo(f"Proxy branch: {proxy_branch}")

    runs_before = github.get_proxy_runs_list(proxy_branch)

    github.run_proxy_dispatches(proxy_branch, github_ref, github_sha, full_test_suite)
    wait_condition(lambda: len(github.get_proxy_runs_list(proxy_branch)) > len(runs_before))

    runs_after = github.get_proxy_runs_list(proxy_branch)
    proxy_run_id = list(set(runs_after) - set(runs_before))[0]
    link = f"https://github.com/neonlabsorg/proxy-model.py/actions/runs/{proxy_run_id}"
    click.echo(f"Proxy run link: {link}")
    click.echo("Waiting completed status...")
    wait_condition(lambda: github.get_proxy_run_info(proxy_run_id)["status"] == "completed", timeout_sec=7200, delay=5)

    if github.get_proxy_run_info(proxy_run_id)["conclusion"] == "success":
        click.echo("Proxy tests passed successfully")
    else:
        raise RuntimeError(f"Proxy tests failed! See {link}")


def wait_condition(func_cond, timeout_sec=15, delay=0.5):
    start_time = time.time()
    while True:
        if time.time() - start_time > timeout_sec:
            return False
        try:
            if func_cond():
                break
        except:
            raise
        time.sleep(delay)
    return True


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


if __name__ == "__main__":
    cli()
