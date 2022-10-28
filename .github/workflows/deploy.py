import os
import re

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
@click.option('--run_number')
def run_tests(github_sha, run_number):
    image_name = f"{IMAGE_NAME}:{github_sha}"

    os.environ["EVM_LOADER_IMAGE"] = image_name
    os.environ["RUN_NUMBER"] = run_number

    command = "docker-compose -f ./evm_loader/docker-compose-test.yml down --timeout 1"
    click.echo(f"run command: {command}")
    subprocess.run(command, shell=True)

    command = "docker-compose -f ./evm_loader/docker-compose-test.yml up -d"
    click.echo(f"run command: {command}")

    subprocess.run(command, stdout=subprocess.PIPE, shell=True)

    try:
        click.echo("start tests")
        exec_id = docker_client.exec_create(
            container=f"solana-{run_number}", cmd="/opt/deploy-test.sh")
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
    proxy_tag = re.sub('\d{1,2}$', 'x',  github_ref.replace("refs/tags/", ""))
    response = requests.get(
        url=f"https://registry.hub.docker.com/v2/repositories/neonlabsorg/proxy/tags/{proxy_tag}")
    if response.status_code != 200:
        raise RuntimeError(
            f"Proxy image with {proxy_tag} tag isn't found. Response: {response.json()}")
    click.echo(f"Proxy image with tag {proxy_tag} is found")


@cli.command(name="trigger_proxy_action")
@click.option('--head_ref_branch')
@click.option('--base_ref_branch')
@click.option('--github_ref')
@click.option('--github_sha')
@click.option('--token')
@click.option('--is_draft')
def trigger_proxy_action(head_ref_branch, base_ref_branch, github_ref, github_sha, token, is_draft):

    if (base_ref_branch == "develop" and not is_draft) or github_ref in ['refs/heads/develop', 'refs/heads/master']:
        full_test_suite = "True"
    else:
        full_test_suite = "False"

    proxy_endpoint = "https://api.github.com/repos/neonlabsorg/proxy-model.py"
    proxy_branches_obj = requests.get(
        f"{proxy_endpoint}/branches?per_page=100").json()
    proxy_branches = [item["name"] for item in proxy_branches_obj]
    proxy_branch = head_ref_branch
    if proxy_branch not in proxy_branches:
        proxy_branch = 'develop'

    neon_evm_branch = head_ref_branch if head_ref_branch is not None else 'develop'

    data = {"ref": proxy_branch,
            "inputs": {"full_test_suite": full_test_suite,
                       "neon_evm_commit": github_sha,
                       "neon_evm_ref": github_ref,
                       "neon_evm_branch": neon_evm_branch}
            }
    headers = {"Authorization": f"Bearer {token}",
               "Accept": "application/vnd.github+json"}
    response = requests.post(
        f"{proxy_endpoint}/actions/workflows/pipeline.yml/dispatches", json=data, headers=headers)
    print(data)
    print(headers)
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
