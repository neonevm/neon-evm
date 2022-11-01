import os
import re
import time

import click
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
    output = docker_client.build(tag=tag, buildargs=buildargs, path="./")

    for line in output:
        if 'stream' in str(line):
            click.echo(str(line).strip('\n'))


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
    if 'refs/tags/' in github_ref:
        tag = github_ref.replace("refs/tags/", "")
    elif github_ref == 'refs/heads/master':
        tag = 'stable'
    elif github_ref == 'refs/heads/develop':
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


@cli.command(name="run_tests")
@click.option('--github_sha')
def run_tests(github_sha):
    image_name = f"{IMAGE_NAME}:{github_sha}"

    os.environ["EVM_LOADER_IMAGE"] = image_name

    command = "docker-compose -f ./evm_loader/docker-compose-test.yml down"
    click.echo(f"run command: {command}")
    subprocess.run(command, shell=True)

    command = "docker-compose -f ./evm_loader/docker-compose-test.yml up -d"
    click.echo(f"run command: {command}")

    subprocess.run(command, stdout=subprocess.PIPE, shell=True)

    try:
        click.echo("start tests")
        exec_id = docker_client.exec_create(
            container="solana", cmd="/opt/deploy-test.sh")
        logs = docker_client.exec_start(exec_id['Id'])
        click.echo(f'logs: {logs}')
        for line in logs:
            if 'ERROR ' in str(line) or 'FAILED ' in str(line):
                raise RuntimeError("Test are failed")

    except:
        raise RuntimeError("Solana container is not run")

    command = "docker-compose -f ./evm_loader/docker-compose-test.yml down --timeout 1"
    click.echo(f"run command: {command}")
    subprocess.run(command, shell=True)


@cli.command(name="check_proxy_tag")
@click.option('--github_ref')
def check_proxy_tag(github_ref):
    proxy_tag = re.sub('\d{1,2}$', 'x', github_ref.replace("refs/tags/", ""))
    response = requests.get(
        url=f"https://registry.hub.docker.com/v2/repositories/neonlabsorg/proxy/tags/{proxy_tag}")
    if response.status_code != 200:
        raise RuntimeError(
            f"Proxy image with {proxy_tag} tag isn't found. Response: {response.json()}")
    click.echo(f"Proxy image with tag {proxy_tag} is found")


@cli.command(name="trigger_proxy_action")
@click.option('--head_ref_branch')
@click.option('--github_ref')
@click.option('--github_sha')
@click.option('--token')
@click.option('--is_draft')
@click.option('--labels')
def trigger_proxy_action(head_ref_branch, github_ref, github_sha, token, is_draft, labels):
    is_develop_branch = github_ref in ['refs/heads/develop', 'refs/heads/master']
    is_tag_creating = 'refs/tags/' in github_ref
    is_version_branch = re.match(r"[vt]{1}\d{1,2}\.\d{1,2}\.x", github_ref.replace("refs/tags/", ""))
    print(labels)
    is_FTS_labeled_not_draft = 'FullTestSuit' in labels and not is_draft

    print(is_develop_branch)
    print(is_tag_creating)
    print(is_version_branch)
    print(is_FTS_labeled_not_draft)
    if is_develop_branch or is_tag_creating or is_version_branch or is_FTS_labeled_not_draft:
        full_test_suite = "true"
    else:
        full_test_suite = "false"

    github = GithubClient(token)

    proxy_branch = head_ref_branch
    if proxy_branch not in github.get_proxy_branches():
        proxy_branch = 'develop'

    runs_before = github.get_proxy_runs_list(proxy_branch)

    github.run_proxy_dispatches(proxy_branch, github_ref, github_sha, full_test_suite)
    wait_condition(lambda: len(github.get_proxy_runs_list(proxy_branch)) > len(runs_before))

    runs_after = github.get_proxy_runs_list(proxy_branch)
    proxy_run_id = list(set(runs_after) - set(runs_before))[0]
    click.echo(f"Proxy run id: {proxy_run_id}")
    click.echo("Waiting completed status...")
    wait_condition(lambda: github.get_proxy_run_info(proxy_run_id)["status"] == "completed", timeout_sec=7200, delay=5)

    if github.get_proxy_run_info(proxy_run_id)["conclusion"] == "success":
        click.echo("Proxy tests passed successfully")
    else:
        raise RuntimeError(f"Proxy tests failed! \
        See https://github.com/neonlabsorg/proxy-model.py/actions/runs/{proxy_run_id}")


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
