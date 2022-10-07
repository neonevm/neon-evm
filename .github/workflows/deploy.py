import os

import click
import docker
import sys
import subprocess
import requests
import json
from urllib.parse import urlparse

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
@click.option('--branch')
@click.option('--github_sha')
def publish_image(branch, github_sha):
    if branch == 'master':
        tag = 'stable'
    elif branch == 'develop':
        tag = 'latest'
    else:
        tag = branch.split('/')[-1]

    docker_client.login(username=DOCKER_USER, password=DOCKER_PASSWORD)

    docker_client.tag(f"{IMAGE_NAME}:{github_sha}", tag)
    docker_client.push(f"{IMAGE_NAME}:{tag}")

    docker_client.tag(f"{IMAGE_NAME}:{github_sha}", github_sha)
    docker_client.push(f"{IMAGE_NAME}:{github_sha}")


@cli.command(name="run_tests")
@click.option('--github_sha')
def run_tests(github_sha):
    image_name = f"{IMAGE_NAME}:{github_sha}"

    os.environ["EVM_LOADER_IMAGE"] = image_name

    command = "docker-compose -f ./evm_loader/docker-compose-test.yml down --timeout 1"
    click.echo(f"run command: {command}")
    subprocess.run(command, shell=True)

    command = "docker-compose -f ./evm_loader/docker-compose-test.yml up -d"
    click.echo(f"run command: {command}")

    subprocess.run(command, stdout=subprocess.PIPE, shell=True)

    try:
        click.echo("start tests")
        logs = docker_client.exec_create(
            container="solana", cmd="/opt/deploy-test.sh")
        click.echo(logs)
    except:
        raise "Solana container is not run"


@cli.command(name="trigger_proxy_action")
@click.option('--branch')
@click.option('--github_sha')
@click.option('--token')
@click.option('--is_draft')
def trigger_proxy_action(branch, github_sha, token, is_draft):

    if branch == "develop" and not is_draft:
        full_test_suite = True
    else:
        full_test_suite = False

    proxy_endpoint = "https://api.github.com/repos/neonlabsorg/proxy-model.py"
    proxy_endpoint = "https://api.github.com/repos/kristinaNikolaeva/playwright_autotests"
    proxy_branches_obj = requests.get(f"{proxy_endpoint}/branches").json()
    proxy_branches = [item["name"] for item in proxy_branches_obj]
    if branch not in proxy_branches:
        branch = 'develop'

    data = {"ref": f"refs/heads/{branch}",
            "inputs": {"full_test_suite": full_test_suite}}
    headers = {'Authorization': f'Bearer {token}',
               'Content-Type': "application/json"}
    response = requests.post(
        f"{proxy_endpoint}/actions/workflows/init.yml/dispatches", json=data, headers=headers)
    print(response)
    print(response.status_code)
    if response.status_code != 204:
        raise "proxy-model.py action is not triggered"


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
